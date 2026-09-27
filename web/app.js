const form = document.querySelector("#search-form");
const input = document.querySelector("#query");
const results = document.querySelector("#results");
const label = document.querySelector("#result-label");
const count = document.querySelector("#result-count");

loadIndexInfo();

form.addEventListener("submit", async (event) => {
  event.preventDefault();
  const query = input.value.trim();
  if (!query) return;
  label.textContent = "Searching...";
  count.textContent = "";
  results.innerHTML =
    '<div class="empty-state"><p>Looking through the local index...</p></div>';
  try {
    const response = await fetch(`/api/search?q=${encodeURIComponent(query)}`);
    if (!response.ok) throw new Error("Search request failed");
    const payload = await response.json();
    renderResults(payload, query);
  } catch (error) {
    label.textContent = "Search unavailable";
    results.innerHTML =
      '<div class="empty-state"><p>Could not reach the local search server.</p></div>';
  }
});

async function loadIndexInfo() {
  try {
    const response = await fetch("/api/info");
    if (!response.ok) return;
    const info = await response.json();
    count.textContent = `${info.document_count} documents indexed`;
  } catch (error) {
    count.textContent = "index unavailable";
  }
}

function renderResults(payload, query) {
  const matches = payload.results || [];
  label.textContent = matches.length
    ? `Results for “${query}”`
    : "No matching documents";
  count.textContent = `${payload.document_count} documents indexed · ${matches.length} ${matches.length === 1 ? "match" : "matches"}`;
  if (!matches.length) {
    results.innerHTML =
      '<div class="empty-state"><span class="empty-mark">∅</span><p>Try a broader term or an exact phrase.</p></div>';
    return;
  }
  results.innerHTML = matches
    .map(
      (match, index) => `
    <article class="result" style="animation-delay:${index * 45}ms">
      <div class="result-top"><div><div class="result-id">${escapeHtml(match.id)}</div><h3>${escapeHtml(match.title)}</h3></div><span class="score">score ${match.score.toFixed(3)}</span></div>
      <p>${escapeHtml(match.body)}</p>
      ${match.phrase_matches && match.phrase_matches.length ? `<span class="phrase">Exact phrase: ${escapeHtml(match.phrase_matches.join(", "))}</span>` : ""}
    </article>`,
    )
    .join("");
}

function escapeHtml(value) {
  return value.replace(
    /[&<>'"]/g,
    (character) =>
      ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", "'": "&#39;", '"': "&quot;" })[
        character
      ],
  );
}
