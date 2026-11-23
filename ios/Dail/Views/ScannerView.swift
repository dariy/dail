import AVFoundation
import SwiftUI
import UIKit

/// How the scanner ended: exactly one of these, once.
enum ScanEnd {
    /// The first decoded QR code's text.
    case scanned(String)
    /// *Cancel*.
    case cancelled
    /// The camera could not run.
    case failed(ScanFailure)
}

/// The in-app QR code scanner (SRV-QR3), full screen: AVFoundation's preview and metadata output,
/// QR codes only. The first decoded text ends it; nothing here looks at what the text says (Rust's
/// `scanned` does). The camera permission was granted before it is shown.
struct ScannerView: View {
    let onEnd: (ScanEnd) -> Void

    var body: some View {
        ZStack {
            Color.black.ignoresSafeArea()
            CameraPreview(onEnd: onEnd).ignoresSafeArea()
            VStack(spacing: 0) {
                VStack(alignment: .leading, spacing: 4) {
                    Text(tr("Scan QR code")).font(.title2)
                    Text(tr("Point the camera at the QR code dail shows on the server."))
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(16)
                .background(Color.black.opacity(0.6))
                Spacer()
                Button(tr("Cancel")) { onEnd(.cancelled) }
                    .frame(maxWidth: .infinity)
                    .padding(16)
                    .background(Color.black.opacity(0.6))
            }
            .foregroundStyle(.white)
        }
    }
}

/// The capture session's controller in SwiftUI.
private struct CameraPreview: UIViewControllerRepresentable {
    let onEnd: (ScanEnd) -> Void

    func makeUIViewController(context: Context) -> ScannerController { ScannerController(onEnd: onEnd) }

    func updateUIViewController(_ controller: ScannerController, context: Context) {}
}

/// An `AVCaptureSession` from the default video camera into a preview layer and a metadata output
/// restricted to `.qr`. The session starts and stops on a queue of its own (both block).
final class ScannerController: UIViewController, AVCaptureMetadataOutputObjectsDelegate {
    private let onEnd: (ScanEnd) -> Void
    private let session = AVCaptureSession()
    private let queue = DispatchQueue(label: "dail-scan")
    private var preview: AVCaptureVideoPreviewLayer?
    /// Touched on main only.
    private var done = false

    init(onEnd: @escaping (ScanEnd) -> Void) {
        self.onEnd = onEnd
        super.init(nibName: nil, bundle: nil)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { return nil }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .black
        guard let device = AVCaptureDevice.default(for: .video) else { return finish(.failed(.noCamera)) }
        guard let input = try? AVCaptureDeviceInput(device: device), session.canAddInput(input) else {
            return finish(.failed(.failed))
        }
        session.addInput(input)
        let output = AVCaptureMetadataOutput()
        guard session.canAddOutput(output) else { return finish(.failed(.failed)) }
        session.addOutput(output)
        output.setMetadataObjectsDelegate(self, queue: .main)
        guard output.availableMetadataObjectTypes.contains(.qr) else { return finish(.failed(.failed)) }
        output.metadataObjectTypes = [.qr]
        let layer = AVCaptureVideoPreviewLayer(session: session)
        layer.videoGravity = .resizeAspectFill
        view.layer.addSublayer(layer)
        preview = layer
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        preview?.frame = view.bounds
        if let connection = preview?.connection, let scene = view.window?.windowScene {
            let angle: CGFloat = switch scene.interfaceOrientation {
            case .landscapeLeft: 180
            case .landscapeRight: 0
            case .portraitUpsideDown: 270
            default: 90
            }
            if connection.isVideoRotationAngleSupported(angle) { connection.videoRotationAngle = angle }
        }
    }

    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        guard preview != nil else { return }
        queue.async { [session] in session.startRunning() }
    }

    override func viewWillDisappear(_ animated: Bool) {
        super.viewWillDisappear(animated)
        queue.async { [session] in session.stopRunning() }
    }

    func metadataOutput(
        _ output: AVCaptureMetadataOutput, didOutput objects: [AVMetadataObject], from connection: AVCaptureConnection
    ) {
        let text = objects.lazy
            .compactMap { $0 as? AVMetadataMachineReadableCodeObject }
            .first { $0.type == .qr && $0.stringValue != nil }?
            .stringValue
        if let text { finish(.scanned(text)) }
    }

    /// Ends the scan once; later codes and errors are dropped.
    private func finish(_ end: ScanEnd) {
        guard !done else { return }
        done = true
        DispatchQueue.main.async { self.onEnd(end) }
    }
}
