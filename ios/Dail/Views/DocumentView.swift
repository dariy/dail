import SwiftUI
import UIKit

/// Reads `blob` requests (`Host.blob`); blocks, so it is called only off the main thread.
struct Blobs {
    var load: (JSON) -> Data? = { _ in nil }
}

private struct BlobsKey: EnvironmentKey {
    static let defaultValue = Blobs()
}

extension EnvironmentValues {
    var blobs: Blobs {
        get { self[BlobsKey.self] }
        set { self[BlobsKey.self] = newValue }
    }
}

/// `blob`'s image answer (width and height as little-endian `u32`, then RGBA) as a `CGImage`.
func rgbaImage(_ data: Data) -> CGImage? {
    guard data.count >= 8 else { return nil }
    let width = Int(data.withUnsafeBytes { $0.loadUnaligned(fromByteOffset: 0, as: UInt32.self).littleEndian })
    let height = Int(data.withUnsafeBytes { $0.loadUnaligned(fromByteOffset: 4, as: UInt32.self).littleEndian })
    guard width > 0, height > 0, data.count - 8 == width * height * 4,
          let provider = CGDataProvider(data: data.subdata(in: 8..<data.count) as CFData)
    else { return nil }
    return CGImage(
        width: width, height: height, bitsPerComponent: 8, bitsPerPixel: 32, bytesPerRow: width * 4,
        space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.last.rawValue),
        provider: provider, decode: nil, shouldInterpolate: true, intent: .defaultIntent)
}

/// What a document view shows: pages in points, how to get a page's base image, and, for a PDF,
/// how to get a tile (nil for a picture, whose base image is all there is).
struct DocumentSource {
    let pages: [CGSize]
    let base: (Int, Int) -> CGImage?
    let tile: ((Int, Int, CGRect) -> CGImage?)?
    let maxSide: Int
    let maxWidth: Int
}

/// Pages in a `UIScrollView`: pinch from fit-width to 5×, double tap fit ↔ 2.5× (not animated under
/// Reduce Motion), pan. Each page draws its fit-width image, and for a PDF a `CATiledLayer` above
/// it asks the host for sharp tiles as the zoom deepens. `onPage` reports the page in view.
struct DocumentView: UIViewRepresentable {
    let source: DocumentSource
    let onPage: (Int) -> Void

    func makeCoordinator() -> Coordinator { Coordinator(source: source, onPage: onPage) }

    func makeUIView(context: Context) -> UIScrollView {
        let scroll = UIScrollView()
        scroll.minimumZoomScale = 1
        scroll.maximumZoomScale = 5
        scroll.contentInsetAdjustmentBehavior = .never
        scroll.backgroundColor = .secondarySystemBackground
        scroll.delegate = context.coordinator
        scroll.addSubview(context.coordinator.content)
        let tap = UITapGestureRecognizer(target: context.coordinator, action: #selector(Coordinator.doubleTap(_:)))
        tap.numberOfTapsRequired = 2
        scroll.addGestureRecognizer(tap)
        scroll.accessibilityIdentifier = "document"
        return scroll
    }

    func updateUIView(_ scroll: UIScrollView, context: Context) {
        context.coordinator.layout(in: scroll)
    }

    final class Coordinator: NSObject, UIScrollViewDelegate {
        let source: DocumentSource
        let onPage: (Int) -> Void
        let content = UIView()
        private var pageViews: [PageView] = []
        private var width: CGFloat = 0
        private var layout = DocumentLayout(pages: [], viewWidth: 0, gap: 0)
        private var current = -1

        init(source: DocumentSource, onPage: @escaping (Int) -> Void) {
            self.source = source
            self.onPage = onPage
        }

        func layout(in scroll: UIScrollView) {
            let w = scroll.bounds.width
            guard w > 0, w != width else { return }
            width = w
            scroll.zoomScale = 1
            layout = DocumentLayout(pages: source.pages, viewWidth: w, gap: source.pages.count > 1 ? 8 : 0)
            content.frame = CGRect(origin: .zero, size: layout.size)
            scroll.contentSize = layout.size
            pageViews.forEach { $0.removeFromSuperview() }
            pageViews = layout.rects.enumerated().map { i, rect in
                let view = PageView(index: i, frame: rect, source: source, scale: scroll.traitCollection.displayScale)
                content.addSubview(view)
                return view
            }
            center(scroll)
            report(scroll)
        }

        func viewForZooming(in scrollView: UIScrollView) -> UIView? { content }

        func scrollViewDidZoom(_ scrollView: UIScrollView) { center(scrollView) }

        func scrollViewDidScroll(_ scrollView: UIScrollView) {
            report(scrollView)
            let visible = CGRect(origin: scrollView.contentOffset, size: scrollView.bounds.size)
            for view in pageViews where view.frame.applying(.init(scaleX: scrollView.zoomScale, y: scrollView.zoomScale)).intersects(visible.insetBy(dx: 0, dy: -visible.height)) {
                view.loadBase()
            }
        }

        /// Centres content smaller than the screen.
        private func center(_ scroll: UIScrollView) {
            let dx = max(0, (scroll.bounds.width - scroll.contentSize.width) / 2)
            let dy = max(0, (scroll.bounds.height - scroll.contentSize.height) / 2)
            scroll.contentInset = UIEdgeInsets(top: dy, left: dx, bottom: dy, right: dx)
        }

        private func report(_ scroll: UIScrollView) {
            let y = (scroll.contentOffset.y + scroll.bounds.height * 0.33) / max(scroll.zoomScale, 0.01)
            let page = layout.nearestPage(y)
            if page != current {
                current = page
                onPage(page)
            }
        }

        @objc func doubleTap(_ gesture: UITapGestureRecognizer) {
            guard let scroll = gesture.view as? UIScrollView else { return }
            let animated = !UIAccessibility.isReduceMotionEnabled
            if scroll.zoomScale < 1.6 {
                let point = gesture.location(in: content)
                let size = CGSize(width: scroll.bounds.width / 2.5, height: scroll.bounds.height / 2.5)
                scroll.zoom(to: CGRect(x: point.x - size.width / 2, y: point.y - size.height / 2, width: size.width, height: size.height), animated: animated)
            } else {
                scroll.setZoomScale(1, animated: animated)
            }
        }
    }
}

/// One page: its fit-width image, loaded off the main thread, and for a PDF the tiles above it.
final class PageView: UIView {
    private let index: Int
    private let source: DocumentSource
    private let pixels: Int
    private let base = UIImageView()
    private var loading = false

    init(index: Int, frame: CGRect, source: DocumentSource, scale: CGFloat) {
        self.index = index
        self.source = source
        pixels = min(Int((frame.width * scale).rounded(.up)), source.maxSide)
        super.init(frame: frame)
        backgroundColor = .white
        base.frame = bounds
        base.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        addSubview(base)
        if source.tile != nil {
            let tiles = TileView(frame: bounds, index: index, source: source)
            tiles.autoresizingMask = [.flexibleWidth, .flexibleHeight]
            addSubview(tiles)
        }
        loadBase()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { nil }

    /// Loads the base image once, on a background queue.
    func loadBase() {
        guard !loading else { return }
        loading = true
        let (index, pixels, source) = (index, pixels, source)
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            guard let image = source.base(index, pixels) else { return }
            DispatchQueue.main.async { self?.base.image = UIImage(cgImage: image) }
        }
    }
}

/// A `CATiledLayer` over a PDF page: each tile at each zoom level comes from the host's `pdf_page`
/// with a tile rectangle. `draw` runs on the layer's background threads, never on main.
final class TileView: UIView {
    private let index: Int
    private let source: DocumentSource

    override class var layerClass: AnyClass { CATiledLayer.self }

    init(frame: CGRect, index: Int, source: DocumentSource) {
        self.index = index
        self.source = source
        super.init(frame: frame)
        isOpaque = false
        let tiled = layer as? CATiledLayer
        tiled?.tileSize = CGSize(width: 512, height: 512)
        // levels for 2×, 4× and 8× the fit width: enough for 5× zoom on any screen scale
        tiled?.levelsOfDetail = 4
        tiled?.levelsOfDetailBias = 3
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { nil }

    override func draw(_ rect: CGRect) {
        guard let tile = source.tile, let context = UIGraphicsGetCurrentContext() else { return }
        // device pixels per point at this level of detail
        let scale = context.ctm.a
        // at fit width the base image is sharp enough
        guard scale > traitCollection.displayScale * 1.2 else { return }
        let width = min(Int((bounds.width * scale).rounded(.up)), source.maxWidth)
        let k = CGFloat(width) / bounds.width
        let height = Int((source.pages[index].height * CGFloat(width) / max(1, source.pages[index].width)).rounded(.down))
        let x = max(0, Int((rect.minX * k).rounded(.down)))
        let y = max(0, Int((rect.minY * k).rounded(.down)))
        let w = min(1024, min(width - x, Int((rect.width * k).rounded(.up))))
        let h = min(1024, min(height - y, Int((rect.height * k).rounded(.up))))
        guard w > 0, h > 0, let image = tile(index, width, CGRect(x: x, y: y, width: w, height: h)) else { return }
        let target = CGRect(x: CGFloat(x) / k, y: CGFloat(y) / k, width: CGFloat(w) / k, height: CGFloat(h) / k)
        UIImage(cgImage: image).draw(in: target)
    }
}
