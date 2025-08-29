use bitflags::bitflags;
use serde::Serialize;
use tinytemplate::escape;
use tracing::error;

#[derive(Debug, Serialize)]
pub struct PreferencesContextContentFlag {
    pub checked: bool,
    pub value: i64,
    pub name: String,
}

bitflags! {
/// A set of preference flags packed in an efficient way
#[derive(Debug, Clone, PartialEq, Eq, Copy)]
    pub struct PreferencesFlags: i64 {
        const BillSigned = 1;
        const BillAccepted = 1 << 1;
        const BillAcceptanceRequested = 1 << 2;
        const BillAcceptanceRejected = 1 << 3;
        const BillAcceptanceTimeout = 1 << 4;
        const BillAcceptanceRecourse = 1 << 5;
        const BillPaymentRequested = 1 << 6;
        const BillPaymentRejected = 1 << 7;
        const BillPaymentTimeout = 1 << 8;
        const BillPaymentRecourse = 1 << 9;
        const BillRecourseRejected = 1 << 10;
        const BillRecourseTimeout = 1 << 11;
        const BillSellOffered = 1 << 12;
        const BillBuyingRejected = 1 << 13;
        const BillPaid = 1 << 14;
        const BillRecoursePaid = 1 << 15;
        const BillEndorsed = 1 << 16;
        const BillSold = 1 << 17;
        const BillMintingRequested = 1 << 18;
        const BillNewQuote = 1 << 19;
        const BillQuoteApproved = 1 << 20;
    }
}

impl PreferencesFlags {
    pub fn as_context_vec(self) -> Vec<PreferencesContextContentFlag> {
        let all_flags = [
            (Self::BillSigned, "Bill Signed"),
            (Self::BillAccepted, "Bill Accepted"),
            (Self::BillAcceptanceRequested, "Bill Acceptance Requested"),
            (Self::BillAcceptanceRejected, "Bill Acceptance Rejected"),
            (Self::BillAcceptanceTimeout, "Bill Acceptance Timeout"),
            (Self::BillAcceptanceRecourse, "Bill Acceptance Recourse"),
            (Self::BillPaymentRequested, "Bill Payment Requested"),
            (Self::BillPaymentRejected, "Bill Payment Rejected"),
            (Self::BillPaymentTimeout, "Bill Payment Timeout"),
            (Self::BillPaymentRecourse, "Bill Payment Recourse"),
            (Self::BillRecourseRejected, "Bill Recourse Rejected"),
            (Self::BillRecourseTimeout, "Bill Recourse Timeout"),
            (Self::BillSellOffered, "Bill Sell Offered"),
            (Self::BillBuyingRejected, "Bill Buying Rejected"),
            (Self::BillPaid, "Bill Paid"),
            (Self::BillRecoursePaid, "Bill Recourse Paid"),
            (Self::BillEndorsed, "Bill Endorsed"),
            (Self::BillSold, "Bill Sold"),
            (Self::BillMintingRequested, "Bill Minting Requested"),
            (Self::BillNewQuote, "Bill New Quote"),
            (Self::BillQuoteApproved, "Bill Quote Approved"),
        ];

        all_flags
            .iter()
            .map(|(flag, name)| PreferencesContextContentFlag {
                checked: self.contains(*flag),
                value: flag.bits(),
                name: name.to_string(),
            })
            .collect()
    }

    pub fn to_title(self) -> String {
        match self {
            Self::BillSigned => "You have been issued an eBill.".to_string(),
            Self::BillAccepted => "An eBill has been accepted.".to_string(),
            Self::BillAcceptanceRequested => {
                "You have been requested to accept an eBill.".to_string()
            }
            Self::BillAcceptanceRejected => "Acceptance of an eBill has been rejected.".to_string(),
            Self::BillAcceptanceTimeout => "Acceptance of an eBill has timed out.".to_string(),
            Self::BillAcceptanceRecourse => {
                "You have been recoursed against on an eBill because of acceptance.".to_string()
            }
            Self::BillPaymentRequested => "You have been requested to pay an eBill.".to_string(),
            Self::BillPaymentRejected => "Payment of an eBill has been rejected.".to_string(),
            Self::BillPaymentTimeout => "Payment of an eBill has timed out.".to_string(),
            Self::BillPaymentRecourse => {
                "You have been recoursed against on an eBill because of payment.".to_string()
            }
            Self::BillRecourseRejected => "Recourse of an eBill has been rejected.".to_string(),
            Self::BillRecourseTimeout => "Recourse of an eBill has timed out.".to_string(),
            Self::BillSellOffered => "You have been offered to buy an eBill.".to_string(),
            Self::BillBuyingRejected => "Buying of an eBill has been rejected.".to_string(),
            Self::BillPaid => "An eBill has been paid".to_string(),
            Self::BillRecoursePaid => "Recourse of an eBill has been paid.".to_string(),
            Self::BillEndorsed => "You have been endorsed an eBill.".to_string(),
            Self::BillSold => "You have bought an eBill.".to_string(),
            Self::BillMintingRequested => "You have been requested to mint an eBill.".to_string(),
            Self::BillNewQuote => "There is a new quote for an eBill.".to_string(),
            Self::BillQuoteApproved => "A quote for an eBill has been approved.".to_string(),
            _ => "You have received a notification.".to_string(), // shouldn't happen, but safe fallback
        }
    }

    pub fn to_link(self, url: &url::Url, id: &str) -> String {
        // currently, we only have bill notifications, so we just create a link to the bill
        let mut path = "/bill/".to_string();
        escape(id, &mut path);
        let link = match url.join(&path) {
            Ok(u) => u,
            Err(e) => {
                error!("error creating to_link: {e}");
                url.to_owned()
            }
        };
        link.to_string()
    }
}

impl Default for PreferencesFlags {
    fn default() -> Self {
        Self::BillSigned
            | Self::BillAccepted
            | Self::BillAcceptanceRequested
            | Self::BillAcceptanceTimeout
            | Self::BillAcceptanceRejected
            | Self::BillAcceptanceRecourse
            | Self::BillPaid
            | Self::BillPaymentRequested
            | Self::BillPaymentTimeout
            | Self::BillPaymentRejected
            | Self::BillPaymentRecourse
            | Self::BillRecoursePaid
            | Self::BillRecourseRejected
            | Self::BillRecourseTimeout
            | Self::BillMintingRequested
    }
}
