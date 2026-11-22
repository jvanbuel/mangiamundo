import UIKit

/// Opens the system share sheet over whatever is on screen, sheets included. Used once the
/// server has made the link, which ShareLink can't wait for.
@MainActor func presentShareSheet(_ items: [Any]) {
    let window = UIApplication.shared.connectedScenes
        .compactMap { ($0 as? UIWindowScene)?.keyWindow }
        .first
    guard var top = window?.rootViewController else { return }
    while let presented = top.presentedViewController { top = presented }
    let sheet = UIActivityViewController(activityItems: items, applicationActivities: nil)
    // On iPad the sheet is a popover and needs an anchor.
    sheet.popoverPresentationController?.sourceView = top.view
    sheet.popoverPresentationController?.sourceRect = CGRect(x: top.view.bounds.midX, y: top.view.bounds.midY, width: 0, height: 0)
    top.present(sheet, animated: true)
}
