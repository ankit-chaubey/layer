/* layer docs — Custom JS */

// Convert special blockquotes into styled callouts
// Usage in markdown:  > **NOTE** text here
document.addEventListener('DOMContentLoaded', function () {

  // ── Callout boxes ─────────────────────────────────────────────
  const callouts = {
    'NOTE':    { cls: 'callout-note',    icon: '📝', label: 'Note' },
    'INFO':    { cls: 'callout-info',    icon: 'ℹ️',  label: 'Info' },
    'TIP':     { cls: 'callout-tip',     icon: '💡', label: 'Tip' },
    'WARNING': { cls: 'callout-warning', icon: '⚠️', label: 'Warning' },
    'DANGER':  { cls: 'callout-danger',  icon: '🚨', label: 'Danger' },
    'SECURITY':{ cls: 'callout-danger',  icon: '🔒', label: 'Security' },
  };

  document.querySelectorAll('blockquote').forEach(bq => {
    const first = bq.querySelector('p:first-child, li:first-child');
    if (!first) return;

    const text = first.textContent.trim();
    for (const [key, meta] of Object.entries(callouts)) {
      if (text.startsWith(key + ':') || text.startsWith('**' + key + '**')) {
        const wrapper = document.createElement('div');
        wrapper.className = 'callout ' + meta.cls;

        const icon = document.createElement('div');
        icon.className = 'callout-icon';
        icon.textContent = meta.icon;

        const content = document.createElement('div');
        content.className = 'callout-content';

        const label = document.createElement('strong');
        label.textContent = meta.label;
        content.appendChild(label);

        // Remove the keyword from first element
        const cleaned = first.innerHTML
          .replace(new RegExp('^\\*\\*' + key + '\\*\\*:?\\s*'), '')
          .replace(new RegExp('^' + key + ':?\\s*'), '');
        first.innerHTML = cleaned;

        // Move all children
        while (bq.firstChild) content.appendChild(bq.firstChild);

        wrapper.appendChild(icon);
        wrapper.appendChild(content);
        bq.parentNode.replaceChild(wrapper, bq);
        break;
      }
    }
  });

  // ── Copy button language label on code blocks ─────────────────
  document.querySelectorAll('pre code[class*="language-"]').forEach(code => {
    const lang = (code.className.match(/language-(\w+)/) || [])[1];
    if (!lang || lang === 'text') return;
    const label = document.createElement('span');
    label.style.cssText = `
      position:absolute; top:10px; right:42px;
      font-size:0.64rem; font-weight:600; letter-spacing:0.08em;
      text-transform:uppercase; color:rgba(255,255,255,0.22);
      font-family:'JetBrains Mono',monospace; pointer-events:none;
    `;
    label.textContent = lang;
    code.parentElement.style.position = 'relative';
    code.parentElement.appendChild(label);
  });

  // ── Smooth scroll for anchor links ────────────────────────────
  document.querySelectorAll('a[href^="#"]').forEach(a => {
    a.addEventListener('click', e => {
      const target = document.querySelector(a.getAttribute('href'));
      if (target) {
        e.preventDefault();
        target.scrollIntoView({ behavior: 'smooth', block: 'start' });
      }
    });
  });
});

  // Force mdBook's font-size slider to start at a comfortable level (3 = medium-large)
  // mdBook stores the value in localStorage under 'mdbook-font-size'
  if (!localStorage.getItem('mdbook-font-size')) {
    localStorage.setItem('mdbook-font-size', '2');
  }

