import { useEffect, useRef, useState } from 'react';

export function usePreviewPlayback() {
  const [playing, setPlaying] = useState(false);
  const [reducedMotion, setReducedMotion] = useState(true);
  const videoRef = useRef<HTMLVideoElement>(null);
  const pausedByUser = useRef(false);

  useEffect(() => {
    const video = videoRef.current;
    if (!video) return;
    const motionPreference = window.matchMedia('(prefers-reduced-motion: reduce)');

    function updatePlayback() {
      if (!video) return;
      setReducedMotion(motionPreference.matches);
      if (document.hidden || motionPreference.matches || pausedByUser.current) video.pause();
      else void video.play().catch(() => { /* O botão permite tentar iniciar se autoplay for bloqueado. */ });
    }

    updatePlayback();
    document.addEventListener('visibilitychange', updatePlayback);
    motionPreference.addEventListener('change', updatePlayback);
    return () => {
      video.pause();
      document.removeEventListener('visibilitychange', updatePlayback);
      motionPreference.removeEventListener('change', updatePlayback);
    };
  }, []);

  function togglePlayback() {
    const video = videoRef.current;
    if (!video) return;
    pausedByUser.current = !video.paused;
    if (video.paused) void video.play().catch(() => {});
    else video.pause();
  }

  return { videoRef, playing, setPlaying, reducedMotion, togglePlayback };
}
