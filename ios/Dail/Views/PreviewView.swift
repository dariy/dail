import SwiftUI

/// A received attachment in the app: a header with Close, the name and Save, then a zoomable
/// picture, PDF pages with "Page n of m", cleaned text, or why it is not shown.
struct PreviewView: View {
    let preview: AttachmentPreview
    let send: (UiEvent) -> Void
    @Environment(\.blobs) private var blobs

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 12) {
                FrameButton(button: preview.close, send: send)
                VStack(alignment: .leading) {
                    Text(preview.name).font(.headline).lineLimit(1)
                    Text(preview.detail).font(.caption).foregroundStyle(.secondary)
                }
                Spacer()
                FrameButton(button: preview.save, send: send)
            }
            .padding(12)
            Divider()
            content.frame(maxWidth: .infinity, maxHeight: .infinity)
        }
        .accessibilityAction(.escape) { send(.tap(preview.close.action)) }
        // the zoom and scroll stay while frames come and go
        .id("\(preview.blob)/\(preview.part)")
    }

    @ViewBuilder private var content: some View {
        switch preview.kind {
        case .image, .pdf:
            DocumentLoader(preview: preview, blobs: blobs)
        case .text:
            TextLoader(preview: preview, blobs: blobs)
        case .none:
            Text(preview.note ?? "").padding(24)
        }
    }
}

/// Loads the pages (or the picture) once, then shows them in a `DocumentView`.
private struct DocumentLoader: View {
    let preview: AttachmentPreview
    let blobs: Blobs
    @State private var source: DocumentSource?
    @State private var failed = false
    @State private var page = 0

    var body: some View {
        Group {
            if let source {
                ZStack(alignment: .bottom) {
                    DocumentView(source: source) { page = $0 }
                    if preview.kind == .pdf {
                        Text("Page \(page + 1) of \(source.pages.count)")
                            .font(.caption)
                            .padding(.horizontal, 12).padding(.vertical, 6)
                            .background(.thinMaterial, in: Capsule())
                            .padding(16)
                            .accessibilityIdentifier("page")
                    }
                }
            } else if failed {
                Text(preview.failed ?? "").padding(24)
            } else {
                ProgressView()
            }
        }
        .task(id: "\(preview.blob)/\(preview.part)") {
            let (preview, blobs) = (preview, blobs)
            let loaded = await Task.detached { Self.load(preview, blobs) }.value
            source = loaded
            failed = loaded == nil
        }
    }

    private static func load(_ preview: AttachmentPreview, _ blobs: Blobs) -> DocumentSource? {
        let blob = JSON.string(preview.blob)
        let part = JSON.number(Double(preview.part))
        if preview.kind == .image {
            guard let data = blobs.load(.object(["image": .object(["blob": blob, "part": part])])),
                  let image = rgbaImage(data)
            else { return nil }
            return DocumentSource(
                pages: [CGSize(width: image.width, height: image.height)], base: { _, _ in image }, tile: nil,
                maxSide: Int.max, maxWidth: 0)
        }
        guard let data = blobs.load(.object(["pdf_pages": .object(["blob": blob, "part": part])])),
              case let .object(answer)? = try? JSONDecoder().decode(JSON.self, from: data),
              case let .array(list)? = answer["pages"],
              case let .number(maxSide)? = answer["max_side"],
              case let .number(maxWidth)? = answer["max_width"]
        else { return nil }
        let pages: [CGSize] = list.compactMap {
            guard case let .array(size) = $0, size.count == 2, case let .number(w) = size[0], case let .number(h) = size[1]
            else { return nil }
            return CGSize(width: w, height: h)
        }
        guard !pages.isEmpty, pages.count == list.count else { return nil }
        let cache = NSCache<NSNumber, CGImage>()
        cache.totalCostLimit = 96 << 20
        func page(_ index: Int, _ width: Int, _ tile: CGRect?) -> CGImage? {
            var args: [String: JSON] = [
                "blob": blob, "part": part, "page": .number(Double(index)), "width": .number(Double(width)),
            ]
            if let tile {
                args["tile"] = .object([
                    "x": .number(tile.minX), "y": .number(tile.minY), "w": .number(tile.width), "h": .number(tile.height),
                ])
            }
            return blobs.load(.object(["pdf_page": .object(args)])).flatMap(rgbaImage)
        }
        return DocumentSource(
            pages: pages,
            base: { index, width in
                if let image = cache.object(forKey: NSNumber(value: index)) { return image }
                guard let image = page(index, width, nil) else { return nil }
                cache.setObject(image, forKey: NSNumber(value: index), cost: image.width * image.height * 4)
                return image
            },
            tile: { index, width, rect in page(index, width, rect) },
            maxSide: Int(maxSide), maxWidth: Int(maxWidth))
    }
}

/// Loads the cleaned text once and shows it monospaced.
private struct TextLoader: View {
    let preview: AttachmentPreview
    let blobs: Blobs
    @State private var text: String?
    @State private var done = false

    var body: some View {
        Group {
            if let text {
                ScrollView {
                    Text(text).font(.system(.footnote, design: .monospaced)).textSelection(.enabled)
                        .frame(maxWidth: .infinity, alignment: .leading).padding(12)
                }
            } else if done {
                Text(preview.failed ?? "").padding(24)
            } else {
                ProgressView()
            }
        }
        .task(id: "\(preview.blob)/\(preview.part)") {
            let request: JSON = .object(["text": .object(["blob": .string(preview.blob), "part": .number(Double(preview.part))])])
            let blobs = blobs
            text = await Task.detached { blobs.load(request).map { String(decoding: $0, as: UTF8.self) } }.value
            done = true
        }
    }
}
