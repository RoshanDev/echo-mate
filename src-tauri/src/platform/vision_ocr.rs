use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct OcrLine {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub confidence: f64,
}

pub fn recognize_text(image_path: &Path) -> anyhow::Result<Vec<OcrLine>> {
    recognize_text_impl(image_path)
}

#[cfg(target_os = "macos")]
fn recognize_text_impl(image_path: &Path) -> anyhow::Result<Vec<OcrLine>> {
    let script_path = swift_script_path();
    std::fs::write(&script_path, MACOS_VISION_SWIFT)?;
    let output = Command::new("/usr/bin/swift")
        .arg(&script_path)
        .arg(image_path)
        .output()
        .map_err(|e| anyhow::anyhow!("无法启动 Apple Vision OCR helper: {e}"))?;
    if !output.status.success() {
        anyhow::bail!(
            "Apple Vision OCR helper failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut lines = Vec::new();
    for line in stdout.lines().filter(|line| !line.trim().is_empty()) {
        let value: serde_json::Value = serde_json::from_str(line)?;
        lines.push(OcrLine {
            text: value
                .get("text")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string(),
            x: json_f64(&value, "x"),
            y: json_f64(&value, "y"),
            width: json_f64(&value, "width"),
            height: json_f64(&value, "height"),
            confidence: json_f64(&value, "confidence").clamp(0.0, 1.0),
        });
    }
    Ok(lines)
}

#[cfg(not(target_os = "macos"))]
fn recognize_text_impl(_image_path: &Path) -> anyhow::Result<Vec<OcrLine>> {
    anyhow::bail!("Apple Vision OCR is only available on macOS")
}

#[cfg(target_os = "macos")]
fn swift_script_path() -> PathBuf {
    std::env::temp_dir().join("echomate-vision-ocr.swift")
}

fn json_f64(value: &serde_json::Value, key: &str) -> f64 {
    value
        .get(key)
        .and_then(|value| value.as_f64())
        .unwrap_or(0.0)
}

#[cfg(target_os = "macos")]
const MACOS_VISION_SWIFT: &str = r#"
import Foundation
import Vision
import AppKit

func emit(_ object: [String: Any]) {
    if let data = try? JSONSerialization.data(withJSONObject: object, options: []),
       let text = String(data: data, encoding: .utf8) {
        print(text)
    }
}

let args = CommandLine.arguments
guard args.count >= 2 else {
    fputs("missing image path\n", stderr)
    exit(2)
}

let path = args[1]
guard let image = NSImage(contentsOfFile: path) else {
    fputs("cannot read image\n", stderr)
    exit(3)
}

var rect = CGRect(origin: .zero, size: image.size)
guard let cgImage = image.cgImage(forProposedRect: &rect, context: nil, hints: nil) else {
    fputs("cannot create cgImage\n", stderr)
    exit(4)
}

let request = VNRecognizeTextRequest()
request.recognitionLevel = .accurate
request.usesLanguageCorrection = false
request.recognitionLanguages = ["zh-Hans", "zh-Hant", "en-US"]

let handler = VNImageRequestHandler(cgImage: cgImage, options: [:])
do {
    try handler.perform([request])
} catch {
    fputs("vision perform failed: \(error)\n", stderr)
    exit(5)
}

let observations = (request.results ?? []).sorted {
    if abs($0.boundingBox.minY - $1.boundingBox.minY) > 0.02 {
        return $0.boundingBox.minY > $1.boundingBox.minY
    }
    return $0.boundingBox.minX < $1.boundingBox.minX
}

for observation in observations {
    guard let top = observation.topCandidates(1).first else { continue }
    let box = observation.boundingBox
    emit([
        "text": top.string,
        "confidence": Double(top.confidence),
        "x": Double(box.minX),
        "y": Double(1.0 - box.maxY),
        "width": Double(box.width),
        "height": Double(box.height)
    ])
}
"#;
