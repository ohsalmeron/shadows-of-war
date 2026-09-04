import Foundation
import RevenueCat
import UIKit

private var configuredAppUserID: String?

/// iOS has no launcher environment, so Rust reads its public endpoints from
/// the signed app bundle through this synchronous bridge.
@_cdecl("sow_ios_config_value")
public func sowIOSConfigValue(
    _ key: UnsafePointer<CChar>?,
    _ buffer: UnsafeMutablePointer<CChar>?,
    _ capacity: Int32
) -> Int32 {
    guard let key, let buffer, capacity > 1,
          let value = Bundle.main.object(
              forInfoDictionaryKey: String(cString: key)
          ) as? String else {
        return 0
    }

    let bytes = Array(value.utf8)
    let count = min(bytes.count, Int(capacity) - 1)
    for index in 0..<count {
        buffer[index] = CChar(bitPattern: bytes[index])
    }
    buffer[count] = 0
    return Int32(count)
}

private func revenueCatAPIKey() -> String? {
    guard let value = Bundle.main.object(forInfoDictionaryKey: "SOWRevenueCatIOSPublicKey") as? String else {
        return nil
    }
    let key = value.trimmingCharacters(in: .whitespacesAndNewlines)
    return key.hasPrefix("appl_") ? key : nil
}

private func topViewController(from controller: UIViewController) -> UIViewController {
    if let presented = controller.presentedViewController {
        return topViewController(from: presented)
    }
    if let navigation = controller as? UINavigationController,
       let visible = navigation.visibleViewController {
        return topViewController(from: visible)
    }
    if let tab = controller as? UITabBarController,
       let selected = tab.selectedViewController {
        return topViewController(from: selected)
    }
    return controller
}

private func presentAlert(from host: UIViewController, title: String, message: String) {
    DispatchQueue.main.async {
        topViewController(from: host).present(
            UIAlertController(title: title, message: message, preferredStyle: .alert),
            animated: true
        )
    }
}

private func configureRevenueCat(
    appUserID: String,
    host: UIViewController,
    completion: @escaping (Bool) -> Void
) {
    guard configuredAppUserID != appUserID else {
        completion(true)
        return
    }
    if Purchases.isConfigured {
        Purchases.shared.logIn(appUserID) { _, _, error in
            if let error {
                presentAlert(from: host, title: "Store unavailable", message: error.localizedDescription)
                completion(false)
                return
            }
            configuredAppUserID = appUserID
            completion(true)
        }
    } else {
        guard let apiKey = revenueCatAPIKey() else {
            presentAlert(
                from: host,
                title: "Store unavailable",
                message: "RevenueCat iOS configuration is missing."
            )
            completion(false)
            return
        }
        Purchases.configure(withAPIKey: apiKey, appUserID: appUserID)
        configuredAppUserID = appUserID
        completion(true)
    }
}

private func scheduleServerBalanceRefresh() {
    // RevenueCat delivers the authoritative grant through a webhook. Refresh
    // a few times so a successful purchase becomes visible without a restart.
    for delay in [0.5, 2.0, 5.0, 10.0] {
        DispatchQueue.main.asyncAfter(deadline: .now() + delay) {
            sow_ios_revenuecat_purchase_completed()
        }
    }
}

private func presentStore(appUserID: String, host: UIViewController) {
    configureRevenueCat(appUserID: appUserID, host: host) { configured in
        guard configured else { return }
        Purchases.shared.getOfferings { offerings, error in
            if let error {
                presentAlert(from: host, title: "Store unavailable", message: error.localizedDescription)
                return
            }

            guard let packages = offerings?.current?.availablePackages, !packages.isEmpty else {
                presentAlert(
                    from: host,
                    title: "Store unavailable",
                    message: "No store packages are available yet."
                )
                return
            }

            DispatchQueue.main.async {
                let host = topViewController(from: host)
                let sheet = UIAlertController(
                    title: "Shadows of War Store",
                    message: "Choose your gem bundle",
                    preferredStyle: .actionSheet
                )
                for package in packages {
                    let product = package.storeProduct
                    sheet.addAction(UIAlertAction(
                        title: "\(product.localizedTitle) — \(product.localizedPriceString)",
                        style: .default
                    ) { _ in
                        Purchases.shared.purchase(package: package) { _, _, error, userCancelled in
                            if userCancelled { return }
                            if let error {
                                presentAlert(
                                    from: host,
                                    title: "Purchase failed",
                                    message: error.localizedDescription
                                )
                            } else {
                                scheduleServerBalanceRefresh()
                                presentAlert(
                                    from: host,
                                    title: "Purchase successful",
                                    message: "Your gems will appear after the game confirms the purchase."
                                )
                            }
                        }
                    })
                }
                sheet.addAction(UIAlertAction(title: "Cancel", style: .cancel))
                if let popover = sheet.popoverPresentationController {
                    popover.sourceView = host.view
                    popover.sourceRect = CGRect(
                        x: host.view.bounds.midX,
                        y: host.view.bounds.midY,
                        width: 0,
                        height: 0
                    )
                }
                host.present(sheet, animated: true)
            }
        }
    }
}

@_cdecl("sow_revenuecat_open_store")
public func sowRevenueCatOpenStore(
    _ appUserID: UnsafePointer<CChar>,
    _ hostViewController: UnsafeMutableRawPointer
) {
    let userID = String(cString: appUserID)
    let host = Unmanaged<UIViewController>.fromOpaque(hostViewController).takeUnretainedValue()
    DispatchQueue.main.async {
        presentStore(appUserID: userID, host: host)
    }
}
