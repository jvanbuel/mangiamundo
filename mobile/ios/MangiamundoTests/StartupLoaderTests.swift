import XCTest
@testable import Mangiamundo

final class StartupLoaderTests: XCTestCase {
    func testBitesTakeAboutASecond() {
        XCTAssertEqual(StartupLoader.total, 0.89, accuracy: 0.0001)
    }

    func testLetsTheBiteInProgressFinish() {
        XCTAssertEqual(StartupLoader.wait(after: 0.1), 0) // before the first bite
        XCTAssertEqual(StartupLoader.wait(after: 0.25), 0.1, accuracy: 0.0001) // first bite growing
        XCTAssertEqual(StartupLoader.wait(after: 0.4), 0) // between bites
        XCTAssertEqual(StartupLoader.biting(0.5), 1)
        XCTAssertEqual(StartupLoader.biting(0.8), 2)
        XCTAssertNil(StartupLoader.biting(0.95)) // all done
    }
}
