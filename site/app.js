document.addEventListener('DOMContentLoaded', () => {
  let currentLang = 'vi';

  const langToggleBtn = document.getElementById('langToggleBtn');
  const langBadge = langToggleBtn.querySelector('.lang-badge');
  const langText = langToggleBtn.querySelector('.lang-text');

  langToggleBtn.addEventListener('click', () => {
    if (currentLang === 'vi') {
      currentLang = 'en';
      langBadge.textContent = 'EN';
      langText.textContent = 'English';
      document.querySelectorAll('.lang-vi').forEach(el => el.classList.add('hidden'));
      document.querySelectorAll('.lang-en').forEach(el => el.classList.remove('hidden'));
    } else {
      currentLang = 'vi';
      langBadge.textContent = 'VI';
      langText.textContent = 'Tiếng Việt';
      document.querySelectorAll('.lang-en').forEach(el => el.classList.add('hidden'));
      document.querySelectorAll('.lang-vi').forEach(el => el.classList.remove('hidden'));
    }
  });

  // Copy code blocks
  document.querySelectorAll('.copy-btn').forEach(btn => {
    btn.addEventListener('click', () => {
      const codeBlock = btn.closest('.code-block').querySelector('code');
      if (codeBlock) {
        navigator.clipboard.writeText(codeBlock.innerText).then(() => {
          const originalText = btn.innerText;
          btn.innerText = 'Copied!';
          setTimeout(() => {
            btn.innerText = originalText;
          }, 2000);
        });
      }
    });
  });

  // Search filtering
  const searchInput = document.getElementById('searchInput');
  searchInput.addEventListener('input', (e) => {
    const query = e.target.value.toLowerCase();
    document.querySelectorAll('.doc-section').forEach(section => {
      const text = section.innerText.toLowerCase();
      if (text.includes(query)) {
        section.style.display = 'block';
      } else {
        section.style.display = 'none';
      }
    });
  });

  // Sidebar link highlight on scroll
  const navLinks = document.querySelectorAll('.nav-link');
  const sections = document.querySelectorAll('.doc-section');

  window.addEventListener('scroll', () => {
    let current = '';
    sections.forEach(section => {
      const sectionTop = section.offsetTop - 80;
      if (window.scrollY >= sectionTop) {
        current = section.getAttribute('id');
      }
    });

    navLinks.forEach(link => {
      link.classList.remove('active');
      if (link.getAttribute('href') === `#${current}`) {
        link.classList.add('active');
      }
    });
  });
});
