// ADVERSARY headless rasteriser: PDF → PNG with CoreGraphics (no window, no
// viewer, no AppKit). Also reports the fonts the PDF uses and how many ink
// pixels are blends of white with a palette colour.
//
//   swiftc -O raster.swift -o raster
//   raster SCALE OUT_DIR a.pdf b.pdf …   → prints one JSON line per PDF
import CoreGraphics
import Foundation
import ImageIO
import UniformTypeIdentifiers

// ntex-base.sty, every \definecolor (Ink, Rule/Back/Head per hue, Slate, Proof greys)
let paletteHex: [(String, UInt32)] = [
    ("Ink", 0x1E1E28),
    ("BlueRule", 0x1E47B8), ("BlueBack", 0xE0E7FB), ("BlueHead", 0x153079),
    ("VioletRule", 0x661EB8), ("VioletBack", 0xECE0FB), ("VioletHead", 0x441579),
    ("TealRule", 0x1C829C), ("TealBack", 0xE2F4F8), ("TealHead", 0x0F596B),
    ("MaroonRule", 0xA92D4A), ("MaroonBack", 0xF8EAED), ("MaroonHead", 0x79152D),
    ("GreenRule", 0x238B45), ("GreenBack", 0xE5F5EA), ("GreenHead", 0x145D2C),
    ("OchreRule", 0xB07D12), ("OchreBack", 0xFAF3E2), ("OchreHead", 0x5E420A),
    ("Slate", 0x5A6378), ("ProofRule", 0x8C8C8C), ("ProofBack", 0xF3F3F3),
]
let palette: [(String, [Double])] = paletteHex.map { (n, h) in
    (n, [Double((h >> 16) & 255), Double((h >> 8) & 255), Double(h & 255)])
}

/// Is p a blend α·c + (1−α)·white of some palette colour (within tol)?
func nearestBlend(_ p: [Double]) -> (String, Double) {
    var best = ("", 1e9)
    for (name, c) in palette {
        let d = c.map { 255 - $0 }
        let q = p.map { 255 - $0 }
        let dd = d.reduce(0) { $0 + $1 * $1 }
        var a = dd > 0 ? zip(q, d).reduce(0) { $0 + $1.0 * $1.1 } / dd : 0
        a = min(1, max(0, a))
        var err = 0.0
        for i in 0..<3 { err = max(err, abs(p[i] - (a * c[i] + (1 - a) * 255))) }
        if err < best.1 { best = (name, err) }
    }
    return best
}

func fonts(_ dict: CGPDFDictionaryRef?, into out: inout Set<String>, depth: Int = 0) {
    guard let dict = dict, depth < 6 else { return }
    var fontDict: CGPDFDictionaryRef?
    if CGPDFDictionaryGetDictionary(dict, "Font", &fontDict), let fd = fontDict {
        CGPDFDictionaryApplyBlock(fd, { _, obj, _ in
            var f: CGPDFDictionaryRef?
            if CGPDFObjectGetValue(obj, .dictionary, &f), let f = f {
                var name: UnsafePointer<Int8>?
                if CGPDFDictionaryGetName(f, "BaseFont", &name), let name = name {
                    out.insert(String(cString: name))
                }
            }
            return true
        }, nil)
    }
    var xo: CGPDFDictionaryRef?
    if CGPDFDictionaryGetDictionary(dict, "XObject", &xo), let xo = xo {
        CGPDFDictionaryApplyBlock(xo, { _, obj, _ in
            var s: CGPDFStreamRef?
            if CGPDFObjectGetValue(obj, .stream, &s), let s = s, let sd = CGPDFStreamGetDictionary(s) {
                var res: CGPDFDictionaryRef?
                if CGPDFDictionaryGetDictionary(sd, "Resources", &res) { fonts(res, into: &out, depth: depth + 1) }
            }
            return true
        }, nil)
    }
}

let args = CommandLine.arguments
let scale = Double(args[1])!
let outDir = URL(fileURLWithPath: args[2])
try? FileManager.default.createDirectory(at: outDir, withIntermediateDirectories: true)
for path in args.dropFirst(3) {
    let url = URL(fileURLWithPath: path)
    guard let doc = CGPDFDocument(url as CFURL), let page = doc.page(at: 1) else {
        print("{\"pdf\":\"\(path)\",\"error\":\"cannot open\"}"); continue
    }
    let box = page.getBoxRect(.mediaBox)
    let w = Int((box.width * scale).rounded(.up)), h = Int((box.height * scale).rounded(.up))
    let cs = CGColorSpaceCreateDeviceRGB()
    guard let ctx = CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4,
                              space: cs, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { continue }
    ctx.setFillColor(CGColor(red: 1, green: 1, blue: 1, alpha: 1))
    ctx.fill(CGRect(x: 0, y: 0, width: w, height: h))
    ctx.scaleBy(x: scale, y: scale)
    ctx.translateBy(x: -box.minX, y: -box.minY)
    ctx.drawPDFPage(page)
    let img = ctx.makeImage()!
    let png = outDir.appendingPathComponent(url.deletingPathExtension().lastPathComponent + ".png")
    if let dest = CGImageDestinationCreateWithURL(png as CFURL, UTType.png.identifier as CFString, 1, nil) {
        CGImageDestinationAddImage(dest, img, nil)
        CGImageDestinationFinalize(dest)
    }
    // palette conformity
    let data = ctx.data!.bindMemory(to: UInt8.self, capacity: w * h * 4)
    var ink = 0, off = 0
    var offenders: [String: Int] = [:]
    var used: [String: Int] = [:]
    for i in stride(from: 0, to: w * h * 4, by: 4) {
        let p = [Double(data[i]), Double(data[i + 1]), Double(data[i + 2])]
        if p.allSatisfy({ $0 > 246 }) { continue }
        ink += 1
        let (name, err) = nearestBlend(p)
        if err > 14 {
            off += 1
            let key = String(format: "%02X%02X%02X", Int(p[0]) & 0xF0, Int(p[1]) & 0xF0, Int(p[2]) & 0xF0)
            offenders[key, default: 0] += 1
        } else { used[name, default: 0] += 1 }
    }
    var fs = Set<String>()
    fonts(page.dictionary.flatMap { d -> CGPDFDictionaryRef? in
        var r: CGPDFDictionaryRef?
        return CGPDFDictionaryGetDictionary(d, "Resources", &r) ? r : nil
    }, into: &fs)
    let top = offenders.sorted { $0.value > $1.value }.prefix(6).map { "\"\($0.key)\":\($0.value)" }.joined(separator: ",")
    let usedS = used.sorted { $0.value > $1.value }.prefix(8).map { "\"\($0.key)\":\($0.value)" }.joined(separator: ",")
    let fontS = fs.sorted().map { "\"\($0)\"" }.joined(separator: ",")
    print("{\"pdf\":\"\(path)\",\"png\":\"\(png.path)\",\"w_pt\":\(box.width),\"h_pt\":\(box.height),\"ink\":\(ink),\"off_palette\":\(off),\"off_samples\":{\(top)},\"palette_used\":{\(usedS)},\"fonts\":[\(fontS)]}")
}
