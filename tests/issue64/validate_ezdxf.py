import ezdxf

# 1. acadrust round-trip already produced output.dxf; validate it the way the reporter did
document = ezdxf.readfile("tests/issue64/byblock_repro_output.dxf")
document.saveas("tests/issue64/resaved.dxf")
print("saveas OK - no DXFTableEntryError")

# audit for good measure
auditor = document.audit()
print(f"audit: errors={auditor.errors}, fixes={auditor.fixes}")

# 2. and the raw input for reference
doc2 = ezdxf.readfile("tests/issue64/byblock_repro_input.dxf")
auditor2 = doc2.audit()
print(f"input audit: errors={auditor2.errors}, fixes={auditor2.fixes}")

# confirm the DIMSTYLE text style resolves to a real STYLE record
msp_style = document.dimstyles.get("Standard")
if msp_style is not None:
    handle = msp_style.dxf.handle
    ts = msp_style.dxf.dimtxsty
    print(f"Standard DIMSTYLE handle={handle}, dimtxsty={ts}")