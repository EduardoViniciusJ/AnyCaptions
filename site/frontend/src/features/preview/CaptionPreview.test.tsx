import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { CaptionPreview } from './CaptionPreview';
import { DictationPreview } from './DictationPreview';

beforeEach(() => {
  vi.stubGlobal('matchMedia', vi.fn(() => ({
    matches: true,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  })));
  vi.stubGlobal('ResizeObserver', class {
    observe() {}
    unobserve() {}
    disconnect() {}
  });
  Element.prototype.scrollIntoView = vi.fn();
  vi.spyOn(HTMLMediaElement.prototype, 'play').mockImplementation(function (this: HTMLMediaElement) {
    Object.defineProperty(this, 'paused', { configurable: true, value: false });
    this.dispatchEvent(new Event('play'));
    return Promise.resolve();
  });
  vi.spyOn(HTMLMediaElement.prototype, 'pause').mockImplementation(function (this: HTMLMediaElement) {
    Object.defineProperty(this, 'paused', { configurable: true, value: true });
    this.dispatchEvent(new Event('pause'));
  });
});

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

it('remove o skeleton ao carregar o vídeo ou falhar, mesmo sem autoplay', () => {
  render(<><CaptionPreview /><DictationPreview /></>);
  const captions = screen.getByRole('region', { name: 'Prévia de transcrição e tradução' });
  const dictation = screen.getByRole('region', { name: 'Transcreva o áudio do seu microfone em texto onde você escreve.' });
  const captionStage = captions.querySelector('.preview-stage')!;
  const dictationStage = dictation.querySelector('.preview-stage')!;

  expect(captionStage.getAttribute('aria-busy')).toBe('true');
  expect(dictationStage.querySelector('[data-slot="skeleton"]')).toBeTruthy();
  expect(HTMLMediaElement.prototype.play).not.toHaveBeenCalled();

  fireEvent.loadedMetadata(captions.querySelector('video')!);
  expect(captionStage.getAttribute('aria-busy')).toBe('false');
  expect(captionStage.querySelector('[data-slot="skeleton"]')).toBeNull();
  expect(dictationStage.getAttribute('aria-busy')).toBe('true');

  fireEvent.error(dictation.querySelector('video')!);
  expect(dictationStage.getAttribute('aria-busy')).toBe('false');
  expect(dictationStage.querySelector('[data-slot="skeleton"]')).toBeNull();
});

it('começa em português e troca a legenda do vídeo usando o teclado', async () => {
  const { container } = render(<CaptionPreview />);
  expect(container.querySelector('.preview-caption')?.textContent).toBe('Eles estão rushando A!');
  const video = container.querySelector('video')!;
  video.currentTime = 3;

  fireEvent.keyDown(screen.getByRole('combobox'), { key: 'ArrowDown' });
  const option = await screen.findByRole('option', { name: 'English' });
  fireEvent.keyDown(option, { key: 'Enter' });

  await waitFor(() => {
    expect(container.querySelector('.preview-caption')?.getAttribute('lang')).toBe('en');
    expect(container.querySelector('.preview-caption')?.textContent).toBe("They're rushing A!");
    expect(screen.getByRole('combobox', { name: 'Idioma da legenda: English' }).getAttribute('aria-expanded')).toBe('false');
    expect(video.currentTime).toBe(0);
  });
});

it('revela as palavras no tempo do vídeo e reinicia o efeito quando o loop volta ao começo', () => {
  vi.mocked(window.matchMedia).mockReturnValue({
    matches: false,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  } as unknown as MediaQueryList);
  const { container } = render(<CaptionPreview />);
  const video = container.querySelector('video')!;
  const caption = container.querySelector('.preview-caption')!;
  const recent = container.querySelector('.caption-recent')!;

  expect(caption.textContent).toBe('Eles');
  video.currentTime = 0.5;
  fireEvent.timeUpdate(video);
  expect(caption.textContent).toBe('Eles estão rushando');
  expect(recent.textContent).toBe('estão rushando');

  fireEvent.click(screen.getByRole('button', { name: 'Pausar demonstração' }));
  expect(caption.textContent).toBe('Eles estão rushando');
  video.currentTime = 1;
  fireEvent.timeUpdate(video);
  expect(caption.textContent).toBe('Eles estão rushando A!');
  expect(recent.textContent).toBe('');

  video.currentTime = 0;
  fireEvent.timeUpdate(video);
  expect(caption.textContent).toBe('Eles');
});

it('respeita movimento reduzido e permite iniciar e pausar manualmente', () => {
  render(<CaptionPreview />);
  expect(HTMLMediaElement.prototype.play).not.toHaveBeenCalled();

  fireEvent.click(screen.getByRole('button', { name: 'Reproduzir demonstração' }));
  expect(HTMLMediaElement.prototype.play).toHaveBeenCalledOnce();
  fireEvent.click(screen.getByRole('button', { name: 'Pausar demonstração' }));
  expect(screen.getByRole('button', { name: 'Reproduzir demonstração' })).toBeTruthy();
});

it('inicia em loop sem som e mantém uma pausa manual ao retornar à página', () => {
  vi.mocked(window.matchMedia).mockReturnValue({
    matches: false,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  } as unknown as MediaQueryList);
  const { container } = render(<CaptionPreview />);
  const video = container.querySelector('video')!;
  expect(video.muted).toBe(true);
  expect(video.loop).toBe(true);
  expect(HTMLMediaElement.prototype.play).toHaveBeenCalledOnce();

  fireEvent.click(screen.getByRole('button', { name: 'Pausar demonstração' }));
  fireEvent(document, new Event('visibilitychange'));
  expect(HTMLMediaElement.prototype.play).toHaveBeenCalledOnce();
  expect(video.paused).toBe(true);
});

it('permite pausar o ditado sem interromper a demonstração de legendas', () => {
  vi.mocked(window.matchMedia).mockReturnValue({
    matches: false,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  } as unknown as MediaQueryList);
  render(<><CaptionPreview /><DictationPreview /></>);
  const captions = screen.getByRole('region', { name: 'Prévia de transcrição e tradução' });
  const dictation = screen.getByRole('region', { name: 'Transcreva o áudio do seu microfone em texto onde você escreve.' });
  const captionVideo = captions.querySelector('video')!;
  const dictationVideo = dictation.querySelector('video')!;

  fireEvent.click(within(dictation).getByRole('button', { name: 'Pausar demonstração de ditado' }));
  fireEvent(document, new Event('visibilitychange'));
  expect(dictationVideo.paused).toBe(true);
  expect(captionVideo.paused).toBe(false);
  expect(within(captions).getByRole('button', { name: 'Pausar demonstração' })).toBeTruthy();

  fireEvent.click(within(dictation).getByRole('button', { name: 'Reproduzir demonstração de ditado' }));
  expect(dictationVideo.paused).toBe(false);
});
