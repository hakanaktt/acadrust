//! AcDbSpline group 70 uses distinct bits for knot parameterization,
//! frame visibility and creation method. SPLINE and HELIX share this body.
use crate::entities::Spline;

const PARAMETERIZATION: i16 = 0x1e0;
const FRAME: i16 = 512;
const FIT_METHOD: i16 = 1024;

pub(super) fn read(spline: &mut Spline, flags: i16) {
    spline.dxf_flags = flags;
    spline.flags.closed = flags & 1 != 0;
    spline.flags.periodic = flags & 2 != 0;
    spline.flags.rational = flags & 4 != 0;
    spline.flags.planar = flags & 8 != 0;
    spline.flags.linear = flags & 16 != 0;
    spline.knot_parameterization = match flags & PARAMETERIZATION {
        32 => 0,
        64 => 1,
        128 => 2,
        256 => 15,
        _ => 0, // Legacy DXF without an explicit method keeps the Chord default.
    };
    spline.cv_frame_visible = flags & FRAME != 0;
    spline.dwg_flags1 &= !15;
    if flags & FIT_METHOD != 0 {
        // Fit data must keep UseKnotParameter when subsequently written to DWG.
        spline.dwg_flags1 |= 1 | 8;
        if spline.flags.closed {
            spline.dwg_flags1 |= 4;
        }
    }
    if spline.cv_frame_visible {
        spline.dwg_flags1 |= 2;
    }
}

pub(super) fn write(spline: &Spline) -> i16 {
    // Refresh mapped fields from current typed values, retaining unrelated bits.
    let mut flags = spline.dxf_flags & !0x7ff;
    let fit_method = match spline.dwg_scenario {
        Some(2) => true,
        Some(1) => false,
        _ => {
            spline.dwg_flags1 & 1 != 0
                || (!spline.fit_points.is_empty() && spline.control_points.is_empty())
        }
    };
    // Parameterization belongs to the fit method. A control-point spline gets
    // it only when its own DXF carried it: a DWG keeps a parameterization on
    // every spline, and writing it for a control-point one sets a bit (256,
    // Custom) other writers leave clear (#149).
    if fit_method || spline.dxf_flags & PARAMETERIZATION != 0 {
        flags |= match spline.knot_parameterization {
            0 => 32,
            1 => 64,
            2 => 128,
            15 => 256,
            _ => spline.dxf_flags & PARAMETERIZATION,
        };
    }
    if fit_method {
        flags |= FIT_METHOD;
    }
    if spline.cv_frame_visible {
        flags |= FRAME;
    }
    if spline.flags.closed {
        flags |= 1;
    }
    if spline.flags.periodic {
        flags |= 2;
    }
    if spline.flags.rational {
        flags |= 4;
    }
    if spline.flags.planar {
        flags |= 8;
    }
    if spline.flags.linear {
        flags |= 16;
    }
    flags
}
