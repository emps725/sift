const form = document.querySelector("#search-form");
const input = document.querySelector("#search-input");
const status = document.querySelector("#status");
const results = document.querySelector("#results");
const searchPanel = document.querySelector(".search-panel");

async function performSearch(query) {
  const trimmedQuery = query.trim();

  if (!trimmedQuery) {
    status.textContent = "";
    results.innerHTML = "";
    searchPanel.classList.remove("has-results");
    return;
  }

  status.textContent = "Searching...";
  results.innerHTML = "";

  try {
    const response = await fetch(`/api/search?q=${encodeURIComponent(trimmedQuery)}`);
    if (!response.ok) {
      throw new Error(`Search failed with status ${response.status}`);
    }

    const data = await response.json();
    status.textContent = `${data.results.length} result${data.results.length === 1 ? "" : "s"}`;
    searchPanel.classList.toggle("has-results", data.results.length > 0);

    if (data.results.length === 0) {
      results.innerHTML = '<div class="empty">No documents matched every search term.</div>';
      return;
    }

    results.innerHTML = data.results.map((result, index) => `
      <article class="result-card">
        <div class="meta">
          <span>${escapeHtml(result.name)}</span>
          <span>Score ${result.score.toFixed(3)}</span>
        </div>
        <h2><span>${index + 1}.</span> ${escapeHtml(result.title)}</h2>
        <p>${escapeHtml(result.snippet)}</p>
      </article>
    `).join("");
  } catch (error) {
    status.textContent = "Search unavailable";
    results.innerHTML = '<div class="empty">The search service could not complete the request.</div>';
  }
}

function escapeHtml(value) {
  const element = document.createElement("span");
  element.textContent = value;
  return element.innerHTML;
}

form.addEventListener("submit", (event) => {
  event.preventDefault();
  const query = input.value;
  const url = query.trim()
    ? `/search?q=${encodeURIComponent(query.trim())}`
    : "/";
  history.replaceState(null, "", url);
  performSearch(query);
});

const initialQuery = document.body.dataset.initialQuery;
if (initialQuery) {
  input.value = initialQuery;
  performSearch(initialQuery);
}
