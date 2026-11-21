package net.darii.dail

import net.darii.dail.ui.UiEvent
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** SRV-QR2: which intent data becomes `pair_link`; the link itself is read in Rust. */
class PairLinkTest {
    @Test
    fun aPairLinkIsSentAsItCame() {
        val link = "dail://pair?v=1&h=home.example&p=7443&c=ABCD2345&fp=00&n=Laptop"
        assertEquals(UiEvent.PairLink(link), pairLink(link))
        assertEquals(UiEvent.PairLink("DAIL://PAIR?v=1"), pairLink("DAIL://PAIR?v=1"))
        assertEquals(UiEvent.PairLink("dail://pair"), pairLink("dail://pair"))
    }

    @Test
    fun otherDataIsIgnored() {
        assertNull(pairLink(null))
        assertNull(pairLink(""))
        assertNull(pairLink("dail://open?x=1"))
        assertNull(pairLink("dail://pairing?v=1"))
        assertNull(pairLink("mailto:bob@example.org"))
        assertNull(pairLink("https://dail://pair?v=1"))
    }
}
