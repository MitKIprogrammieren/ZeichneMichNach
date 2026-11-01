// zmn-ocr: Texterkennung für ZeichneMichNach mit Apple Vision (lokal, ohne Internet).
// Aufruf: zmn-ocr <bild>   Ausgabe: JSON-Liste der Textzeilen, Koordinaten 0…1, y von oben.
import AppKit
import Foundation
import Vision

let args = CommandLine.arguments
guard args.count > 1,
      let img = NSImage(contentsOfFile: args[1]),
      let cg = img.cgImage(forProposedRect: nil, context: nil, hints: nil)
else {
    FileHandle.standardError.write("Bild nicht lesbar\n".data(using: .utf8)!)
    exit(1)
}

let req = VNRecognizeTextRequest()
req.recognitionLevel = .accurate
req.usesLanguageCorrection = false
req.recognitionLanguages = ["de-DE", "en-US"]
let handler = VNImageRequestHandler(cgImage: cg, options: [:])
do {
    try handler.perform([req])
} catch {
    FileHandle.standardError.write("\(error)\n".data(using: .utf8)!)
    exit(2)
}
var out: [[String: Any]] = []
for o in req.results ?? [] {
    guard let c = o.topCandidates(1).first else { continue }
    let b = o.boundingBox
    out.append([
        "text": c.string,
        "conf": Double(c.confidence),
        "x": Double(b.minX),
        "y": Double(1 - b.maxY),
        "w": Double(b.width),
        "h": Double(b.height),
    ])
}
let data = try! JSONSerialization.data(withJSONObject: out)
print(String(data: data, encoding: .utf8)!)
