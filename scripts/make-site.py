"""Writes the site's four pages into site/, sharing one header and footer."""
import pathlib
import shutil

SITE = pathlib.Path("site")
(SITE / "assets").mkdir(parents=True, exist_ok=True)
shutil.copy("assets/brand/clipcut-wordmark.png", SITE / "assets" / "wordmark.png")
shutil.copy("assets/icons/clipcut_logo_256.png", SITE / "assets" / "icon.png")
shutil.copy("assets/icons/clipcut_logo_64.png", SITE / "assets" / "favicon.png")

RELEASES = "https://github.com/Realestkai/ClipCut/releases/latest"
ISSUES = "https://github.com/Realestkai/ClipCut/issues/new/choose"


def page(name, title, description, body, here):
    def link(href, label):
        cls = ' class="here"' if href == here else ""
        return f'<a href="{href}"{cls}>{label}</a>'

    html = f"""<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{title}</title>
  <meta name="description" content="{description}">
  <meta property="og:title" content="{title}">
  <meta property="og:description" content="{description}">
  <meta property="og:image" content="assets/icon.png">
  <link rel="icon" href="assets/favicon.png">
  <link rel="preconnect" href="https://fonts.googleapis.com">
  <link href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;650;800&display=swap" rel="stylesheet">
  <link rel="stylesheet" href="style.css">
</head>
<body>
  <header class="top">
    <div class="wrap">
      <a class="brand" href="./"><img src="assets/wordmark.png" alt="ClipCut"></a>
      <nav class="nav">
        {link("./#download", "Download")}
        {link("docs.html", "Docs")}
        {link("whats-new.html", "What's new")}
        {link("report.html", "Report a problem")}
      </nav>
    </div>
  </header>
{body}
  <footer>
    <div class="wrap">
      <span>ClipCut is free software under AGPL-3.0-or-later.</span>
      <span class="right">
        <a href="docs.html">Docs</a>
        <a href="whats-new.html">What's new</a>
        <a href="report.html">Report a problem</a>
        <a href="https://github.com/Realestkai/ClipCut">Source code</a>
      </span>
    </div>
  </footer>
  <script src="site.js"></script>
</body>
</html>
"""
    (SITE / name).write_text(html, encoding="utf-8", newline="\n")


FEATURES = [
    ("💬", "Auto-captions", "Made on your machine. Pick a model size, get styled captions on the timeline."),
    ("🗣️", "Text-to-speech", "A free local voice, or clone any voice from a few seconds of a recording."),
    ("🧍", "Background removal", "People, objects, or paint the mask yourself."),
    ("🎞️", "Keyframes", "Position, scale, rotation, opacity, volume and effect parameters, with a curve editor."),
    ("✨", "100 effects and transitions", "Filters, effects and transitions, live in the preview."),
    ("✂️", "Fast cutting", "Split, trim, ripple, merge, freeze frame, speed. Magnetic timeline if you want it."),
    ("🎚️", "Multi-track", "Overlays, picture-in-picture, blend modes, crop and flips on as many tracks as you like."),
    ("📝", "Titles", "Fonts, stroke, shadow, background plate. Presets to start from."),
    ("🎙️", "Voice cleanup", "Denoise, enhance voice and level the loudness with one switch."),
    ("📤", "Export", "H.264, HEVC and AV1, up to 4K 60 and 10-bit colour."),
    ("🔒", "100% local", "Nothing uploads. Your footage never leaves your disk, and it works offline."),
    ("🌍", "26 languages", "The whole editor, translated."),
]

cards = "\n".join(
    f'        <div class="card"><span class="ico">{ico}</span><h3>{name}</h3><p>{text}</p></div>'
    for ico, name, text in FEATURES
)

page("index.html", "ClipCut - the free video editor for Windows and Mac",
     "ClipCut is a free video editor for Windows and macOS. Auto-captions, effects, keyframes and 4K export, with no watermark, no account and no subscription.",
     f"""  <main>
    <section class="hero">
      <div class="wrap">
        <img class="mark" src="assets/icon.png" alt="">
        <h1>Edit videos. Keep everything.</h1>
        <p class="lead">ClipCut is a free video editor for Windows and Mac. No watermark, no account, no subscription, and your footage never leaves your computer.</p>
        <div class="buttons" id="download">
          <a class="btn" data-os="windows" data-download="windows-x86_64-setup.exe" href="{RELEASES}">Download for Windows <small data-size></small></a>
          <a class="btn" data-os="mac" data-download="macos-arm64.dmg" href="{RELEASES}">Mac, Apple silicon <small data-size></small></a>
          <a class="btn" data-download="macos-x86_64.dmg" href="{RELEASES}">Mac, Intel <small data-size></small></a>
        </div>
        <p class="under">Version <span data-version>latest</span> &middot; Windows 10 or newer &middot; macOS 12 or newer &middot; <a href="docs.html#install">First-launch help</a></p>
      </div>
    </section>

    <section>
      <div class="wrap">
        <h2>Everything most edits need</h2>
        <p class="sub">Made for TikToks, Reels and Shorts, YouTube videos, tutorials, podcast clips and memes.</p>
        <div class="grid">
{cards}
        </div>
      </div>
    </section>

    <section>
      <div class="wrap">
        <h2>Up and cutting in a minute</h2>
        <p class="sub">No sign-up screen stands between you and the timeline.</p>
        <div class="steps">
          <div class="card"><h3>Install it</h3><p>Download the installer for your computer and run it.</p></div>
          <div class="card"><h3>Drop in footage</h3><p>Start a project and drag your videos, photos and music in.</p></div>
          <div class="card"><h3>Cut, caption, export</h3><p>Trim on the timeline, add captions and effects, and export up to 4K.</p></div>
        </div>
        <p class="under" style="text-align:left">ClipCut updates itself: open <strong>Settings &rsaquo; Version</strong> and press <strong>Check for updates</strong>.</p>
      </div>
    </section>
  </main>""", "./#download")

page("docs.html", "ClipCut documentation",
     "How to install ClipCut, make your first edit, add captions, export, update and automate.",
     """  <main class="page">
    <div class="wrap">
      <h1>Documentation</h1>
      <p class="lead">The short version of everything you need to get going.</p>
      <div class="doc">
        <nav class="toc">
          <a href="#install">Install</a>
          <a href="#first-edit">Your first edit</a>
          <a href="#captions">Captions and voices</a>
          <a href="#export">Export</a>
          <a href="#shortcuts">Shortcuts</a>
          <a href="#updates">Updates</a>
          <a href="#automation">Automation</a>
          <a href="#privacy">Privacy</a>
        </nav>
        <div class="prose">
          <h2 id="install">Install</h2>
          <p><a href="./#download">Download</a> the file for your computer.</p>
          <ul>
            <li><strong>Windows:</strong> run the <code>setup.exe</code>. ClipCut is not code-signed yet, so Windows SmartScreen may stop it: press <strong>More info</strong>, then <strong>Run anyway</strong>.</li>
            <li><strong>Mac:</strong> open the <code>.dmg</code> and drag ClipCut into Applications. If macOS refuses to open it, run this once in Terminal:<br><code>xattr -dr com.apple.quarantine /Applications/ClipCut.app</code></li>
          </ul>
          <p>Pick <em>Apple silicon</em> for an M1 or newer Mac, and <em>Intel</em> for an older one. Not sure? Apple menu &rsaquo; About This Mac says which chip you have.</p>

          <h2 id="first-edit">Your first edit</h2>
          <ol>
            <li>Open ClipCut and start a new project from the launch screen.</li>
            <li>Drag videos, photos and music into the media panel, then onto the timeline.</li>
            <li>Move the playhead to where a cut should be and split the clip. Drag a clip's edges to trim it.</li>
            <li>Stack clips on higher tracks for overlays and picture-in-picture.</li>
            <li>Add titles, effects, filters and transitions from the library and adjust them in the inspector.</li>
            <li>Press export when it looks right.</li>
          </ol>
          <div class="note">Projects save to your own disk. Nothing is uploaded anywhere.</div>

          <h2 id="captions">Captions and voices</h2>
          <p>Captions, text-to-speech and background removal run on AI models that live on your computer. They are not bundled with the installer, to keep it small: download the ones you want once from <strong>Settings</strong> (the <strong>Transcriber</strong> and <strong>Speech</strong> pages), and they work offline from then on.</p>
          <p>Bigger caption models are more accurate and slower. Start with a small one and move up if it mishears.</p>

          <h2 id="export">Export</h2>
          <p>ClipCut exports H.264, HEVC and AV1, up to 4K at 60 frames per second, with 10-bit colour where the format allows. For social media, H.264 at the resolution you shot in is the safe choice.</p>

          <h2 id="shortcuts">Shortcuts</h2>
          <p>Every key is listed, and can be changed, in <strong>Settings &rsaquo; Shortcuts</strong>.</p>

          <h2 id="updates">Updates</h2>
          <p>Open <strong>Settings &rsaquo; Version</strong> and press <strong>Check for updates</strong>. ClipCut downloads the new version, checks it is intact and installs it over the one you are running. The same page lets you go back to an earlier version. <a href="whats-new.html">What's new</a> lists what each version changed.</p>

          <h2 id="automation">Automation</h2>
          <p>ClipCut can be driven by scripts and AI agents. Turn it on in <strong>Settings &rsaquo; Automation</strong>: it listens on your own computer only (<code>127.0.0.1</code>) and answers only callers that hold its token. AI agents connect over MCP by starting the app with <code>--mcp</code>.</p>

          <h2 id="privacy">Privacy</h2>
          <p>There is no account and no telemetry. ClipCut reaches the network only when you ask it to: checking for updates, downloading a model, or turning Automation on.</p>
        </div>
      </div>
    </div>
  </main>""", "docs.html")

page("whats-new.html", "What's new in ClipCut",
     "What changed in each version of ClipCut.",
     """  <main class="page">
    <div class="wrap">
      <h1>What's new</h1>
      <p class="lead">What changed in each version. To get the latest, open <strong>Settings &rsaquo; Version</strong> in ClipCut, or <a href="./#download">download it</a>.</p>
      <div id="releases"><p>Loading&hellip;</p></div>
    </div>
  </main>""", "whats-new.html")

page("report.html", "Report a problem with ClipCut",
     "How to report a bug in ClipCut so it can be fixed.",
     f"""  <main class="page">
    <div class="wrap">
      <h1>Report a problem</h1>
      <p class="lead">Something broke, looks wrong or closed by itself? Send these four things and it can usually be fixed.</p>
      <div class="steps">
        <div class="card"><h3>What you did and what happened</h3><p>The steps that lead to the problem, what you expected, and what you got instead.</p></div>
        <div class="card"><h3>Your version</h3><p>It is on <strong>Settings &rsaquo; About</strong>, and <strong>Settings &rsaquo; Version</strong> says whether a newer one is out. Try the newest first.</p></div>
        <div class="card"><h3>The log</h3><p><strong>Settings &rsaquo; About &rsaquo; Show the log</strong> opens the folder: attach the newest file. If ClipCut closed by itself, start it again first and attach the file before the newest.</p></div>
        <div class="card"><h3>Your computer</h3><p><strong>Settings &rsaquo; About &rsaquo; Copy system information</strong>, pasted into the report.</p></div>
      </div>
      <p style="margin-top:28px"><a class="btn primary" href="{ISSUES}">Open a report</a></p>
      <p class="under" style="text-align:left">Reports are public, so leave out anything private. A free account is needed to post one.</p>
    </div>
  </main>""", "report.html")
print("site written:", sorted(p.name for p in SITE.iterdir()))
