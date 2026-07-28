/* Silhouette lab UI — talks to m0-silhouette lab HTTP API. */

const $ = (id) => document.getElementById(id);

let state = null;
let applying = false;

async function api(method, path, body) {
  const opts = { method, headers: {} };
  if (body !== undefined) {
    opts.headers["Content-Type"] = "application/json";
    opts.body = JSON.stringify(body);
  }
  const res = await fetch(path, opts);
  const text = await res.text();
  let data;
  try {
    data = JSON.parse(text);
  } catch {
    throw new Error(text || res.statusText);
  }
  if (!res.ok) {
    throw new Error(data.error || text || res.statusText);
  }
  return data;
}

function setStatus(el, msg, kind) {
  el.textContent = msg || "";
  el.classList.remove("ok", "err");
  if (kind) el.classList.add(kind);
}

function renderSessionMeta(s) {
  $("session-meta").innerHTML =
    `session <strong>${escapeHtml(s.session)}</strong><br>` +
    `next render ${String(s.next_render).padStart(4, "0")} · canvas ${s.size}px<br>` +
    `subjects ${s.subjects.map((x) => x.id).join(", ")}`;
}

function uniqueFamilies(s) {
  return s.families || [];
}

function fieldsForFamily(s, family) {
  if (!family) return s.fields || [];
  return (s.fields || []).filter((f) => f.family === family);
}

function populateFamily(s) {
  const sel = $("family");
  const families = uniqueFamilies(s);
  const current = s.family || "";
  sel.innerHTML =
    `<option value="">— choose family —</option>` +
    families
      .map(
        (f) =>
          `<option value="${escapeHtml(f)}" ${f === current ? "selected" : ""}>${escapeHtml(f)}</option>`
      )
      .join("");
}

function populateParameter(s) {
  const sel = $("parameter");
  const family = s.family || $("family").value || "";
  const fields = fieldsForFamily(s, family);
  const current = s.parameter || "";
  sel.innerHTML =
    `<option value="">— choose parameter —</option>` +
    fields
      .map(
        (f) =>
          `<option value="${escapeHtml(f.path)}" ${f.path === current ? "selected" : ""}>${escapeHtml(f.path)}</option>`
      )
      .join("");
  updateSlider(s);
}

function currentField(s) {
  const path = s.parameter || $("parameter").value;
  if (!path) return null;
  return (s.fields || []).find((f) => f.path === path) || null;
}

function updateSlider(s) {
  const field = currentField(s);
  const slider = $("slider");
  const input = $("value-input");
  const apply = $("apply-value");
  if (!field) {
    $("param-label").textContent = "—";
    $("param-value").textContent = "—";
    $("param-unit").textContent = "";
    $("soft-min").textContent = "";
    $("soft-max").textContent = "";
    slider.disabled = true;
    input.disabled = true;
    apply.disabled = true;
    return;
  }
  $("param-label").textContent = field.path;
  $("param-value").textContent = String(field.value);
  $("param-unit").textContent = field.unit;
  $("soft-min").textContent = String(field.soft_min);
  $("soft-max").textContent = String(field.soft_max);
  slider.min = field.soft_min;
  slider.max = field.soft_max;
  slider.step = field.step;
  slider.value = field.value;
  slider.disabled = false;
  input.min = field.soft_min;
  input.max = field.soft_max;
  input.step = field.step;
  input.value = field.value;
  input.disabled = false;
  apply.disabled = false;
}

function renderDiff(s) {
  const diff = s.table_diff || {};
  const keys = Object.keys(diff);
  if (!keys.length) {
    $("diff").textContent = "(no changes from baseline yet)";
    return;
  }
  $("diff").textContent = keys
    .map((k) => `${k}: ${diff[k].from} → ${diff[k].to}`)
    .join("\n");
}

function parseMeta(raw) {
  if (!raw) return null;
  try {
    return JSON.parse(raw);
  } catch {
    return null;
  }
}

function renderGallery(s) {
  const root = $("gallery");
  const renders = [...(s.renders || [])].reverse();
  const chosenSel = $("chosen-render");
  const prevChosen = chosenSel.value;

  chosenSel.innerHTML =
    `<option value="">— pick a render —</option>` +
    renders
      .map(
        (r) =>
          `<option value="${escapeHtml(r.id)}" ${r.id === prevChosen ? "selected" : ""}>${escapeHtml(r.id)}</option>`
      )
      .join("");

  if (!renders.length) {
    root.innerHTML = `<p class="hint">No renders yet. Lock a family, set a value, click Render strip.</p>`;
    return;
  }

  root.innerHTML = renders
    .map((r) => {
      const meta = parseMeta(r.meta_raw) || {};
      const subjects = meta.subjects || {};
      const ids = Object.keys(subjects);
      const strip = ids
        .map((id) => {
          const info = subjects[id] || {};
          const png = info.png || `${id}.png`;
          const digest = (info.short_digest || info.skeleton_digest || "").slice(0, 16);
          const src = `/files/renders/${encodeURIComponent(r.id)}/${encodeURIComponent(png)}?t=${Date.now()}`;
          return `<figure class="subject">
            <img src="${src}" alt="${escapeHtml(id)}" loading="lazy" data-full="${src}" />
            <figcaption class="cap">${escapeHtml(id)} · ${escapeHtml(digest)}</figcaption>
          </figure>`;
        })
        .join("");
      const title = `${meta.family || "?"} / ${meta.parameter || "?"}`;
      const notes = meta.notes ? ` · ${meta.notes}` : "";
      return `<article class="render-card" data-id="${escapeHtml(r.id)}">
        <header>
          <span class="id">${escapeHtml(r.id)}</span>
          <span class="meta">${escapeHtml(title)}${escapeHtml(notes)}</span>
        </header>
        <div class="strip">${strip}</div>
      </article>`;
    })
    .join("");

  root.querySelectorAll(".render-card").forEach((card) => {
    card.addEventListener("click", (ev) => {
      if (ev.target.tagName === "IMG") {
        window.open(ev.target.dataset.full || ev.target.src, "_blank");
        return;
      }
      const id = card.dataset.id;
      chosenSel.value = id;
      root.querySelectorAll(".render-card").forEach((c) => c.classList.remove("selected"));
      card.classList.add("selected");
    });
  });
}

function applyState(s) {
  state = s;
  // Re-fetch fields always includes only locked family when family set — server filters.
  // For family dropdown we need full family list from s.families.
  renderSessionMeta(s);
  populateFamily(s);
  // When family is set, s.fields is filtered; for parameter list that's what we want.
  // Rebuild parameter options from s.fields.
  populateParameter(s);
  renderDiff(s);
  renderGallery(s);
}

async function refresh() {
  const s = await api("GET", "/api/session");
  applyState(s);
}

async function onFamilyChange() {
  const family = $("family").value;
  try {
    const s = await api("PUT", "/api/family", { family });
    applyState(s);
  } catch (e) {
    setStatus($("render-status"), e.message, "err");
  }
}

async function onParameterChange() {
  const parameter = $("parameter").value;
  try {
    const s = await api("PUT", "/api/parameter", { parameter });
    applyState(s);
  } catch (e) {
    setStatus($("render-status"), e.message, "err");
  }
}

async function commitValue(value) {
  if (!state || !state.parameter && !$("parameter").value) {
    setStatus($("render-status"), "Choose a parameter first", "err");
    return;
  }
  const path = state.parameter || $("parameter").value;
  applying = true;
  try {
    const s = await api("PUT", "/api/field", { path, value: Number(value) });
    applyState(s);
    setStatus($("render-status"), `Set ${path} = ${value}`, "ok");
  } catch (e) {
    setStatus($("render-status"), e.message, "err");
    // reload authoritative value
    try { await refresh(); } catch (_) { /* ignore */ }
  } finally {
    applying = false;
  }
}

async function onRender() {
  const btn = $("render-btn");
  btn.disabled = true;
  setStatus($("render-status"), "Rendering… (first extract can take a while)", "");
  try {
    // Ensure focused field value is committed from the slider.
    if ($("parameter").value && !$("slider").disabled) {
      await commitValue($("slider").value);
    }
    const s = await api("POST", "/api/render", {
      notes: $("render-notes").value || "",
    });
    applyState(s);
    const last = String(s.next_render - 1).padStart(4, "0");
    setStatus($("render-status"), `Wrote renders/${last}`, "ok");
    $("chosen-render").value = last;
  } catch (e) {
    setStatus($("render-status"), e.message, "err");
  } finally {
    btn.disabled = false;
  }
}

async function onExport() {
  const chosen = $("chosen-render").value;
  if (!chosen) {
    setStatus($("export-status"), "Pick a chosen render", "err");
    return;
  }
  try {
    const result = await api("POST", "/api/export", {
      family: state.family || $("family").value,
      parameter: state.parameter || $("parameter").value,
      verdict: $("verdict").value,
      notes: $("export-notes").value || "",
      chosen_render: chosen,
    });
    setStatus(
      $("export-status"),
      `Exported ${result.relative}`,
      "ok"
    );
    await refresh();
  } catch (e) {
    setStatus($("export-status"), e.message, "err");
  }
}

function escapeHtml(s) {
  return String(s)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function wire() {
  $("family").addEventListener("change", onFamilyChange);
  $("parameter").addEventListener("change", onParameterChange);
  $("slider").addEventListener("input", () => {
    if (applying) return;
    $("param-value").textContent = $("slider").value;
    $("value-input").value = $("slider").value;
  });
  $("slider").addEventListener("change", () => {
    commitValue($("slider").value);
  });
  $("apply-value").addEventListener("click", () => {
    commitValue($("value-input").value);
  });
  $("value-input").addEventListener("keydown", (e) => {
    if (e.key === "Enter") commitValue($("value-input").value);
  });
  $("render-btn").addEventListener("click", onRender);
  $("export-btn").addEventListener("click", onExport);
}

wire();
refresh().catch((e) => {
  $("session-meta").textContent = `Failed to load session: ${e.message}`;
});
