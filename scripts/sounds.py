"""The sound library's list: audio/sounds.csv in, audio/manifest.json out.

    python scripts/sounds.py build            # write audio/manifest.json
    python scripts/sounds.py sheet            # write audio/sounds.csv from the manifest

To add a sound (see docs/AUDIO_LIBRARY.md): upload the file into audio/ and
add its row to audio/sounds.csv. The Sounds workflow runs this, uploads the
files to the audio-v1 release and commits the manifest; every copy of
ClipCut reads the manifest the first time its Audio page is opened in a
session, so the sound shows up without an update.

A row the app would not offer is left out and said so: a file that is not
there, no name, or a licence that does not let anyone use the sound in a
video (CC0, CC-BY with an author to credit, or "ClipCut" for our own).
"""
import csv
import hashlib
import json
import pathlib
import re
import shutil
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
AUDIO = ROOT / "audio"
SHEET = AUDIO / "sounds.csv"
MANIFEST = AUDIO / "manifest.json"
COLUMNS = ["file", "name", "category", "author", "license", "source"]
EXTENSIONS = {".wav", ".mp3", ".ogg", ".flac", ".m4a", ".aac", ".opus"}


def slug(text: str) -> str:
    return re.sub(r"-+", "-", re.sub(r"[^a-z0-9]+", "-", text.lower())).strip("-")


def usable(license: str, author: str) -> bool:
    license = license.strip().upper()
    if license in {"CC0", "CC0-1.0", "CLIPCUT"}:
        return True
    return license.startswith("CC-BY") and "NC" not in license and "ND" not in license and bool(author.strip())


def duration(path: pathlib.Path) -> float:
    if not shutil.which("ffprobe"):
        return 0.0
    out = subprocess.run(
        ["ffprobe", "-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", str(path)],
        capture_output=True, text=True, check=False,
    ).stdout.strip()
    try:
        return round(float(out), 2)
    except ValueError:
        return 0.0


def sha256(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def build() -> int:
    old = {}
    if MANIFEST.is_file():
        for sound in json.loads(MANIFEST.read_text(encoding="utf-8")).get("sounds", []):
            old[sound["file"]] = sound
    sounds, problems, ids = [], [], set()
    with SHEET.open(encoding="utf-8-sig", newline="") as handle:
        for line, row in enumerate(csv.DictReader(handle), start=2):
            row = {key: (row.get(key) or "").strip() for key in COLUMNS}
            if not row["file"]:
                continue
            path = AUDIO / row["file"]
            if "/" in row["file"] or "\\" in row["file"] or not path.is_file():
                problems.append(f"line {line}: {row['file']} is not in audio/")
                continue
            if path.suffix.lower() not in EXTENSIONS:
                problems.append(f"line {line}: {row['file']} is not a sound file")
                continue
            if not row["name"]:
                problems.append(f"line {line}: {row['file']} has no name")
                continue
            if not usable(row["license"], row["author"]):
                problems.append(
                    f"line {line}: {row['file']}: licence {row['license']!r} is not one anyone can use in a video "
                    "(CC0, CC-BY with an author, or ClipCut)"
                )
                continue
            sound_id = slug(path.stem)
            while sound_id in ids:
                sound_id += "-2"
            ids.add(sound_id)
            digest = sha256(path)
            previous = old.get(row["file"], {})
            sounds.append({
                "id": previous.get("id", sound_id) if previous.get("sha256") == digest else sound_id,
                "name": row["name"],
                "category": row["category"] or "Other",
                "file": row["file"],
                "duration": previous["duration"] if previous.get("sha256") == digest and previous.get("duration")
                else duration(path),
                "bytes": path.stat().st_size,
                "license": row["license"],
                "source": row["source"],
                "author": row["author"],
                "sha256": digest,
            })
    listed = {sound["file"] for sound in sounds}
    for path in sorted(AUDIO.iterdir()):
        if path.suffix.lower() in EXTENSIONS and path.name not in listed:
            problems.append(f"{path.name} is in audio/ but has no row in sounds.csv, so it is not listed")
    MANIFEST.write_text(json.dumps({"version": 1, "sounds": sounds}, indent=2, ensure_ascii=False) + "\n",
                        encoding="utf-8")
    print(f"{MANIFEST.relative_to(ROOT)}: {len(sounds)} sounds")
    for problem in problems:
        print(f"::warning::{problem}")
    return 0


def sheet() -> int:
    data = json.loads(MANIFEST.read_text(encoding="utf-8"))
    with SHEET.open("w", encoding="utf-8", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=COLUMNS)
        writer.writeheader()
        for sound in data["sounds"]:
            writer.writerow({key: sound.get(key, "") for key in COLUMNS})
    print(f"{SHEET.relative_to(ROOT)}: {len(data['sounds'])} rows")
    return 0


if __name__ == "__main__":
    command = sys.argv[1] if len(sys.argv) > 1 else ""
    if command == "build":
        sys.exit(build())
    if command == "sheet":
        sys.exit(sheet())
    print(__doc__)
    sys.exit(2)
