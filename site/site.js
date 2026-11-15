// The site's one script: the download buttons and the What's new page both
// read the list of releases, so a new release shows up here by itself.
const REPO = "Realestkai/ClipCut";
const RELEASES = `https://github.com/${REPO}/releases`;
const API = `https://api.github.com/repos/${REPO}/releases?per_page=30`;

// Only the app's own releases: "v1.2.3". The models release is not one.
const isAppRelease = (release) =>
  !release.draft && !release.prerelease && /^v\d+\.\d+\.\d+$/.test(release.tag_name);

async function releases() {
  const response = await fetch(API, { headers: { Accept: "application/vnd.github+json" } });
  if (!response.ok) throw new Error(`GitHub answered ${response.status}`);
  return (await response.json()).filter(isAppRelease);
}

function asset(release, ending) {
  const found = release.assets.find((file) => file.name.endsWith(ending));
  return found ? found.browser_download_url : `${RELEASES}/latest`;
}

function megabytes(release, ending) {
  const found = release.assets.find((file) => file.name.endsWith(ending));
  return found ? `${Math.round(found.size / 1048576)} MB` : "";
}

async function fillDownloads() {
  const buttons = document.querySelectorAll("[data-download]");
  if (!buttons.length) return;
  try {
    const [latest] = await releases();
    if (!latest) return;
    for (const button of buttons) {
      const ending = button.dataset.download;
      button.href = asset(latest, ending);
      const size = button.querySelector("[data-size]");
      if (size) size.textContent = megabytes(latest, ending);
    }
    for (const spot of document.querySelectorAll("[data-version]")) {
      spot.textContent = latest.tag_name.slice(1);
    }
  } catch {
    // The buttons already point at the releases page; leave them there.
  }
  // The visitor's own system first.
  const mac = /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent);
  const first = document.querySelector(mac ? "[data-os=mac]" : "[data-os=windows]");
  if (first) {
    first.classList.add("primary");
    first.parentElement.prepend(first);
  }
}

// A release's notes are a little Markdown: headings, bullets, bold, code
// and links. Everything else is shown as it is written.
function escapeHtml(text) {
  return text.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);
}

function inline(text) {
  return escapeHtml(text)
    .replace(/`([^`]+)`/g, "<code>$1</code>")
    .replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>")
    .replace(/\[([^\]]+)\]\((https?:[^)\s]+)\)/g, '<a href="$2">$1</a>');
}

function notes(markdown) {
  const out = [];
  let list = false;
  const close = () => { if (list) { out.push("</ul>"); list = false; } };
  for (const raw of markdown.split(/\r?\n/)) {
    const line = raw.trimEnd();
    // The download table and what follows it is for the releases page.
    if (/^#{2,3}\s+Download\b/.test(line)) break;
    if (/^##\s/.test(line)) continue; // the title, shown above already
    if (/^###\s/.test(line)) { close(); out.push(`<h3>${inline(line.replace(/^###\s+/, ""))}</h3>`); continue; }
    if (/^[-*]\s/.test(line)) {
      if (!list) { out.push("<ul>"); list = true; }
      out.push(`<li>${inline(line.replace(/^[-*]\s+/, ""))}</li>`);
      continue;
    }
    close();
    if (line) out.push(`<p>${inline(line)}</p>`);
  }
  close();
  return out.join("\n");
}

// A version's highlights: site/whats-new/<version>.json, the file the app
// shows in its own What's new dialog. Most versions have none.
async function highlights(version) {
  try {
    const response = await fetch(`whats-new/${version}.json`);
    if (!response.ok) return "";
    const items = (await response.json()).items || [];
    if (!items.length) return "";
    return `<ul class="highlights">${items.map((item) =>
      `<li><strong>${escapeHtml(item.title || "")}</strong>${item.text ? `<span>${escapeHtml(item.text)}</span>` : ""}</li>`).join("")}</ul>`;
  } catch {
    return "";
  }
}

async function fillNews() {
  const spot = document.querySelector("#releases");
  if (!spot) return;
  try {
    const all = await releases();
    if (!all.length) { spot.innerHTML = "<p>No releases yet.</p>"; return; }
    const top = await Promise.all(all.map((release) => highlights(release.tag_name.slice(1))));
    spot.innerHTML = all.map((release, index) => {
      const when = new Date(release.published_at).toLocaleDateString(undefined, { year: "numeric", month: "long", day: "numeric" });
      const body = notes(release.body || "") || "<p>Fixes and polish.</p>";
      return `<article class="release" id="${release.tag_name}">
        <h2>ClipCut ${release.tag_name.slice(1)}${index === 0 ? '<span class="tag">Latest</span>' : ""}</h2>
        <div class="when">${when}</div>
        ${top[index]}
        ${body}
      </article>`;
    }).join("\n");
    if (location.hash) document.getElementById(location.hash.slice(1))?.scrollIntoView();
  } catch {
    spot.innerHTML = `<p>The list of releases could not be loaded. <a href="${RELEASES}">See every release</a>.</p>`;
  }
}

fillDownloads();
fillNews();
