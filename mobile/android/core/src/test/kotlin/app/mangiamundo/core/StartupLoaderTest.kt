package app.mangiamundo.core

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class StartupLoaderTest {
    @Test
    fun bitesTakeAboutASecond() {
        assertEquals(890L, StartupLoader.TOTAL_MS)
    }

    @Test
    fun stayWhileLoading() {
        assertTrue(StartupLoader.keep(ready = false, elapsedMs = 0))
        assertTrue(StartupLoader.keep(ready = false, elapsedMs = 5_000))
    }

    @Test
    fun alwaysPlayAllBitesOnce() {
        assertTrue(StartupLoader.keep(ready = true, elapsedMs = 0)) // data already in
        assertTrue(StartupLoader.keep(ready = true, elapsedMs = 400)) // between bites
        assertTrue(StartupLoader.keep(ready = true, elapsedMs = 889)) // last bite growing
        assertFalse(StartupLoader.keep(ready = true, elapsedMs = 890)) // all done
    }
}
