"""Convert the upstream proportional Zen Hei face to WOFF2."""

from argparse import ArgumentParser
from io import BytesIO
from pathlib import Path
from tarfile import open as open_tar

from fontTools.ttLib import TTFont

parser = ArgumentParser(description=__doc__)
parser.add_argument("output", type=Path)
args = parser.parse_args()

source = Path(__file__).with_name("fonts-wqy-zenhei_0.9.45.orig.tar.gz")
with open_tar(source) as archive:
    with archive.extractfile("wqy-zenhei/wqy-zenhei.ttc") as collection:
        font_data = collection.read()

with TTFont(BytesIO(font_data), fontNumber=0, recalcTimestamp=False) as font:
    if font["name"].getDebugName(6) != "WenQuanYiZenHei":
        raise ValueError("Expected the proportional WenQuanYi Zen Hei face")
    font.flavor = "woff2"
    font.save(args.output)
