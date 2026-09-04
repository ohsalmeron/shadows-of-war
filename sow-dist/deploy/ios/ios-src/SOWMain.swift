// Winit owns UIApplicationMain on iOS. This process entry point must call
// into Rust before UIKit has created UIApplication.shared.
@main
enum SOWMain {
    static func main() {
        sow_ios_main()
    }
}
