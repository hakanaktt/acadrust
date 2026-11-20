//! Survey figures and networks.

/// A survey figure (`AECC_SVFIGURE`) of a survey network: the figure a
/// survey point chain draws, named by its figure name.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SurveyFigure {
    /// The figure's name (`LOT1`, `EP4`, `SHD3`, ...).
    pub name: String,
    /// The name of the survey site the figure belongs to.
    pub site: String,
}

/// A survey figure's name and site (its text stream reads `AeccImpSvFigure`,
/// ``, ``, name, site name).
pub(super) fn figure(strings: &[String]) -> Option<SurveyFigure> {
    Some(SurveyFigure { name: strings.get(3)?.clone(), site: strings.get(4)?.clone() })
        .filter(|f| !f.name.is_empty())
}

/// A survey network's (`AECC_SVNETWORK`) name: its text stream's third
/// string.
pub(super) fn network_name(strings: &[String]) -> Option<String> {
    strings.get(2).filter(|s| !s.is_empty()).cloned()
}
