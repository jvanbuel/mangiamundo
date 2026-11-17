import XCTest
@testable import Mangiamundo

final class StartupLoaderTests: XCTestCase {
    func testBitesTakeAboutASecond() {
        XCTAssertEqual(StartupLoader.total, 0.89, accuracy: 0.0001)
    }

    func testAlwaysPlaysAllBitesOnce() {
        XCTAssertEqual(StartupLoader.wait(after: 0), 0.89, accuracy: 0.0001) // data already in
        XCTAssertEqual(StartupLoader.wait(after: 0.4), 0.49, accuracy: 0.0001)
        XCTAssertEqual(StartupLoader.wait(after: 0.95), 0) // all done
    }
}
