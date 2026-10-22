const phrases: Record<string, string[]> = {
  en: ['The captions follow every word you hear.', 'Stay focused. Your team is waiting on the other side.', 'All the audio stays on your computer.'],
  'pt-BR': ['As legendas acompanham cada palavra que você ouve.', 'Mantenha o foco. Sua equipe espera do outro lado.', 'Todo o áudio fica no seu computador.'],
  es: ['Los subtítulos siguen cada palabra que escuchas.', 'Concéntrate. Tu equipo espera al otro lado.', 'Todo el audio se queda en tu computadora.'],
  zh: ['字幕跟随你听到的每一个字。', '保持专注。你的队友正在另一边等你。', '所有音频都保留在你的电脑上。'],
  fr: ['Les sous-titres suivent chaque mot que vous entendez.', 'Restez concentré. Votre équipe vous attend de l’autre côté.', 'Tout l’audio reste sur votre ordinateur.'],
  ru: ['Субтитры следуют за каждым словом, которое вы слышите.', 'Сосредоточьтесь. Ваша команда ждёт на другой стороне.', 'Всё аудио остаётся на вашем компьютере.'],
};

export function setupCaptionPreview(root: HTMLElement) {
  const trigger = root.querySelector<HTMLButtonElement>('.language-trigger')!;
  const menu = root.querySelector<HTMLElement>('.language-menu')!;
  const picker = root.querySelector<HTMLElement>('.language-picker')!;
  const options = [...menu.querySelectorAll<HTMLButtonElement>('[data-language]')];
  const text = root.querySelector<HTMLElement>('[data-caption-text]')!;
  const reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)');
  let language = 'en';
  let phraseIndex = 0;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let visible = true;

  function setOpen(open: boolean) {
    menu.hidden = !open;
    trigger.setAttribute('aria-expanded', String(open));
  }

  function play() {
    clearTimeout(timer);
    if (!visible) return;
    const phrase = phrases[language][phraseIndex];
    text.lang = language;
    if (reducedMotion.matches) {
      text.textContent = phrase;
      return;
    }
    const words = language === 'zh' ? [...phrase] : phrase.split(' ');
    let count = 1;
    function write() {
      text.replaceChildren();
      const committed = document.createElement('span');
      const recent = document.createElement('span');
      recent.className = 'caption-recent';
      const separator = language === 'zh' ? '' : ' ';
      const split = Math.max(0, count - 2);
      committed.textContent = words.slice(0, split).join(separator) + (split ? separator : '');
      recent.textContent = words.slice(split, count).join(separator);
      text.append(committed, recent);
      if (count++ < words.length) timer = setTimeout(write, 230);
      else {
        text.textContent = phrase;
        timer = setTimeout(() => { phraseIndex = (phraseIndex + 1) % phrases.en.length; play(); }, 3800);
      }
    }
    write();
  }

  trigger.addEventListener('click', () => {
    setOpen(menu.hidden);
    if (!menu.hidden) options.find(option => option.getAttribute('aria-selected') === 'true')?.focus();
  });
  options.forEach(option => option.addEventListener('click', () => {
    language = option.dataset.language!;
    options.forEach(item => item.setAttribute('aria-selected', String(item === option)));
    const image = trigger.querySelector('img')!;
    image.src = `/flags/${option.dataset.languageFlag}.svg`;
    trigger.querySelector('[data-language-label]')!.textContent = option.dataset.languageName!;
    trigger.setAttribute('aria-label', `Idioma da legenda: ${option.dataset.languageName}`);
    setOpen(false);
    trigger.focus();
    play();
  }));
  root.addEventListener('keydown', event => {
    if (menu.hidden && event.target === trigger && ['ArrowDown', 'ArrowUp'].includes(event.key)) {
      event.preventDefault();
      setOpen(true);
      options.find(option => option.getAttribute('aria-selected') === 'true')?.focus();
      return;
    }
    if (event.key === 'Escape') { setOpen(false); trigger.focus(); }
    if (!menu.hidden && ['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) {
      event.preventDefault();
      const index = options.indexOf(document.activeElement as HTMLButtonElement);
      const next = event.key === 'Home' ? 0 : event.key === 'End' ? options.length - 1
        : (index + (event.key === 'ArrowDown' ? 1 : -1) + options.length) % options.length;
      options[next].focus();
    }
    if (event.key === 'Tab' && !menu.hidden) { setOpen(false); trigger.focus(); }
  });
  document.addEventListener('click', event => { if (!picker.contains(event.target as Node)) setOpen(false); });
  document.addEventListener('visibilitychange', () => { visible = !document.hidden; play(); });
  reducedMotion.addEventListener('change', play);
  play();
}
