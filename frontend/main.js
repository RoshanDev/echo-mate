const { invoke } = window.__TAURI__.core || window.__TAURI__;

// Listen for candidate results from Rust backend
window.addEventListener("DOMContentLoaded", () => {
  console.log("EchoMate frontend loaded");

  // Expose handler for Tauri to call when candidates are ready
  window.__echoMateSetCandidates = (candidates) => {
    const cards = document.querySelectorAll(".candidate-card");
    candidates.forEach((c, i) => {
      if (cards[i]) {
        cards[i].querySelector(".candidate-text").textContent = c.text;
        cards[i].querySelector(".tone").textContent =
          `${c.tone} · ${c.strategy}`;
      }
    });
    document.getElementById("status").textContent = "点击「复制」选择回复";
  };

  // Copy button handlers
  document.querySelectorAll(".copy-btn").forEach((btn, i) => {
    btn.addEventListener("click", () => {
      const text = document
        .querySelectorAll(".candidate-text")[i].textContent;
      navigator.clipboard.writeText(text).then(() => {
        btn.textContent = "已复制!";
        setTimeout(() => (btn.textContent = "复制"), 1500);
      });
    });
  });
});
