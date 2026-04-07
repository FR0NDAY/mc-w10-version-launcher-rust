use crate::error::Result;
use time::{format_description::well_known::Rfc3339, Duration, OffsetDateTime};

const DEFAULT_URL: &str = "https://fe3.delivery.mp.microsoft.com/ClientWebService/client.asmx";
const SECURED_URL: &str = "https://fe3.delivery.mp.microsoft.com/ClientWebService/client.asmx/secured";

const SOAP_NS: &str = "http://www.w3.org/2003/05/soap-envelope";
const ADDRESSING_NS: &str = "http://www.w3.org/2005/08/addressing";
const SECEXT_NS: &str = "http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-wssecurity-secext-1.0.xsd";
const SECUTIL_NS: &str = "http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-wssecurity-utility-1.0.xsd";
const WUWS_NS: &str = "http://schemas.microsoft.com/msus/2014/10/WindowsUpdateAuthorization";
const WUCLIENT_NS: &str = "http://www.microsoft.com/SoftwareDistribution/Server/ClientWebService";

const DEVICE_ATTRIBUTES: &str = "E:BranchReadinessLevel=CBB&DchuNvidiaGrfxExists=1&ProcessorIdentifier=Intel64%20Family%206%20Model%2063%20Stepping%202&CurrentBranch=rs4_release&DataVer_RS5=1942&FlightRing=Retail&AttrDataVer=57&InstallLanguage=en-US&DchuAmdGrfxExists=1&OSUILocale=en-US&InstallationType=Client&FlightingBranchName=&Version_RS5=10&UpgEx_RS5=Green&GStatus_RS5=2&OSSkuId=48&App=WU&InstallDate=1529700913&ProcessorManufacturer=GenuineIntel&AppVer=10.0.17134.471&OSArchitecture=AMD64&UpdateManagementGroup=2&IsDeviceRetailDemo=0&HidOverGattReg=C%3A%5CWINDOWS%5CSystem32%5CDriverStore%5CFileRepository%5Chidbthle.inf_amd64_467f181075371c89%5CMicrosoft.Bluetooth.Profiles.HidOverGatt.dll&IsFlightingEnabled=0&DchuIntelGrfxExists=1&TelemetryLevel=1&DefaultUserRegion=244&DeferFeatureUpdatePeriodInDays=365&Bios=Unknown&WuClientVer=10.0.17134.471&PausedFeatureStatus=1&Steam=URL%3Asteam%20protocol&Free=8to16&OSVersion=10.0.17134.472&DeviceFamily=Windows.Desktop";

#[derive(Debug, Default)]
pub struct WUProtocol {
    msa_user_token: Option<String>,
}

impl WUProtocol {
    pub fn set_msa_user_token(&mut self, token: String) {
        self.msa_user_token = Some(token);
    }

    pub fn download_url(&self) -> &'static str {
        SECURED_URL
    }

    pub fn build_download_request(&self, update_identity: &str, revision_number: &str) -> Result<String> {
        let now = OffsetDateTime::now_utc();
        let created = now.format(&Rfc3339)?;
        let expires = (now + Duration::minutes(5)).format(&Rfc3339)?;
        let tickets = self.build_wu_tickets();

        Ok(format!(
            r#"<s:Envelope xmlns:a=\"{addressing}\" xmlns:s=\"{soap}\">
  <s:Header>
    <a:Action s:mustUnderstand=\"1\">{wuclient}/GetExtendedUpdateInfo2</a:Action>
    <a:MessageID>urn:uuid:5754a03d-d8d5-489f-b24d-efc31b3fd32d</a:MessageID>
    <a:To s:mustUnderstand=\"1\">{url}</a:To>
    <o:Security s:mustUnderstand=\"1\" xmlns:o=\"{secext}\">
      <wsu:Timestamp xmlns:wsu=\"{secutil}\">
        <wsu:Created>{created}</wsu:Created>
        <wsu:Expires>{expires}</wsu:Expires>
      </wsu:Timestamp>
      {tickets}
    </o:Security>
  </s:Header>
  <s:Body>
    <GetExtendedUpdateInfo2 xmlns=\"{wuclient}\">
      <updateIDs>
        <UpdateIdentity>
          <UpdateID>{update_identity}</UpdateID>
          <RevisionNumber>{revision_number}</RevisionNumber>
        </UpdateIdentity>
      </updateIDs>
      <infoTypes>
        <XmlUpdateFragmentType>FileUrl</XmlUpdateFragmentType>
      </infoTypes>
      <deviceAttributes>{device_attributes}</deviceAttributes>
    </GetExtendedUpdateInfo2>
  </s:Body>
</s:Envelope>"#,
            addressing = ADDRESSING_NS,
            soap = SOAP_NS,
            wuclient = WUCLIENT_NS,
            url = SECURED_URL,
            secext = SECEXT_NS,
            secutil = SECUTIL_NS,
            created = created,
            expires = expires,
            tickets = tickets,
            update_identity = update_identity,
            revision_number = revision_number,
            device_attributes = DEVICE_ATTRIBUTES,
        ))
    }

    pub fn extract_download_response_urls(xml: &str) -> Result<Vec<String>> {
        let doc = roxmltree::Document::parse(xml)?;
        let mut urls = Vec::new();
        for node in doc.descendants().filter(|n| n.is_element()) {
            if node.tag_name().name() == "Url" {
                if let Some(parent) = node.parent_element() {
                    if parent.tag_name().name() != "FileLocation" {
                        continue;
                    }
                }
                if let Some(text) = node.text() {
                    urls.push(text.to_string());
                }
            }
        }
        Ok(urls)
    }

    fn build_wu_tickets(&self) -> String {
        let mut tickets = String::new();
        tickets.push_str(&format!(
            "<wuws:WindowsUpdateTicketsToken wsu:id=\"ClientMSA\" xmlns:wsu=\"{secutil}\" xmlns:wuws=\"{wuws}\">",
            secutil = SECUTIL_NS,
            wuws = WUWS_NS
        ));
        if let Some(token) = &self.msa_user_token {
            tickets.push_str(
                &format!(
                    "<TicketType Name=\"MSA\" Version=\"1.0\" Policy=\"MBI_SSL\"><User>{}</User></TicketType>",
                    token
                )
            );
        }
        tickets.push_str("<TicketType Name=\"AAD\" Version=\"1.0\" Policy=\"MBI_SSL\"></TicketType>");
        tickets.push_str("</wuws:WindowsUpdateTicketsToken>");
        tickets
    }
}

#[allow(dead_code)]
pub fn default_url() -> &'static str {
    DEFAULT_URL
}
