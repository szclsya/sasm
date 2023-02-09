use super::{PkgVersion, PkgVersionSegment};
use std::cmp::{Ord, Ordering, max};

/// the rpmvercmp algorithm
/// Check https://fedoraproject.org/wiki/Archive:Tools/RPM/VersionComparison
impl Ord for PkgVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        if self.epoch > other.epoch {
            return Ordering::Greater;
        }

        if self.epoch < other.epoch {
            return Ordering::Less;
        }

        let this_segments: Vec<&PkgVersionSegment> = self.version.iter().filter(|x| !matches!(x, PkgVersionSegment::Separater(_))).collect();
        let that_segments: Vec<&PkgVersionSegment> = other.version.iter().filter(|x| !matches!(x, PkgVersionSegment::Separater(_))).collect();

        let max_len = max(this_segments.len(), that_segments.len());
        for i in 0..max_len {
            let this = this_segments.get(i);
            let that = that_segments.get(i);

            match this {
                Some(PkgVersionSegment::Alphabetic(this_val)) => {
                    match that {
                        Some(PkgVersionSegment::Alphabetic(that_val)) => {
                            if this_val > that_val {
                                return Ordering::Greater;
                            } else if this_val < that_val {
                                return Ordering::Less;
                            }
                        },
                        Some(PkgVersionSegment::Number(that_val)) => {
                            return Ordering::Less;
                        },
                        Some(PkgVersionSegment::Separater(_)) => {
                            unreachable!()
                        },
                        None => {
                            return Ordering::Less;
                        }
                    }
                },
                Some(PkgVersionSegment::Number(this_val)) => {
                    match that {
                        Some(PkgVersionSegment::Alphabetic(that_val)) => {
                            return Ordering::Greater;
                        },
                        Some(PkgVersionSegment::Number(that_val)) => {
                            if this_val > that_val {
                                return Ordering::Greater;
                            } else if this_val < that_val {
                                return Ordering::Less;
                            }
                        },
                        Some(PkgVersionSegment::Separater(_)) => {
                            unreachable!()
                        },
                        None => {
                            return Ordering::Greater;
                        }
                    }
                },
                Some(PkgVersionSegment::Separater(_)) => {
                    unreachable!()
                },
                None => {
                    match that {
                        Some(PkgVersionSegment::Alphabetic(that_val)) => {
                            return Ordering::Less;
                        },
                        Some(PkgVersionSegment::Number(that_val)) => {
                            return Ordering::Less;
                        },
                        Some(PkgVersionSegment::Separater(_)) => {
                            unreachable!()
                        },
                        None => (),
                    }
                },
            }
        }

        if self.revision.is_some() && other.revision.is_some() {
            self.revision.cmp(&other.revision)
        } else {
            Ordering::Equal
        }
    }
}

impl PartialOrd for PkgVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
