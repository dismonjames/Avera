document.addEventListener('DOMContentLoaded', () => {
  let lang = 'vi';

  const btn  = document.getElementById('langToggleBtn');
  const badge = btn && btn.querySelector('.lang-badge');
  const ltxt  = btn && btn.querySelector('.lang-text');

  function applyLang(l) {
    lang = l;
    if (badge) badge.textContent = l === 'vi' ? 'VI' : 'EN';
    if (ltxt)  ltxt.textContent  = l === 'vi' ? 'Tiếng Việt' : 'English';
    document.querySelectorAll('.vi').forEach(e => e.classList.toggle('hidden', l !== 'vi'));
    document.querySelectorAll('.en').forEach(e => e.classList.toggle('hidden', l !== 'en'));
  }

  if (btn) btn.addEventListener('click', () => applyLang(lang === 'vi' ? 'en' : 'vi'));
  applyLang('vi');

  // Copy buttons
  document.querySelectorAll('.copy-btn').forEach(btn => {
    btn.addEventListener('click', () => {
      const code = btn.closest('.cb').querySelector('code');
      if (!code) return;
      navigator.clipboard.writeText(code.innerText).then(() => {
        const orig = btn.textContent;
        btn.textContent = 'Copied!';
        setTimeout(() => btn.textContent = orig, 1800);
      });
    });
  });

  // Active nav link on scroll
  const links    = document.querySelectorAll('.nav-link[href^="#"]');
  const sections = document.querySelectorAll('.doc-section[id]');
  if (links.length && sections.length) {
    const onScroll = () => {
      let cur = '';
      sections.forEach(s => { if (window.scrollY >= s.offsetTop - 80) cur = s.id; });
      links.forEach(l => l.classList.toggle('active', l.getAttribute('href') === '#' + cur));
    };
    window.addEventListener('scroll', onScroll, { passive: true });
  }

  // Search filter (single-page only)
  const si = document.getElementById('searchInput');
  if (si) {
    si.addEventListener('input', () => {
      const q = si.value.toLowerCase();
      document.querySelectorAll('.doc-section').forEach(s => {
        s.style.display = (!q || s.innerText.toLowerCase().includes(q)) ? '' : 'none';
      });
    });
  }
});
