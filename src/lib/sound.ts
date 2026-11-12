// Un „clinchet” scurt, sintetizat (fără fișiere audio).
let ctx: AudioContext | null = null;

export function playChime() {
  try {
    ctx ??= new AudioContext();
    const t0 = ctx.currentTime;
    const master = ctx.createGain();
    master.gain.value = 0.12;
    master.connect(ctx.destination);
    [1046.5, 1568, 2093].forEach((f, i) => {
      const o = ctx!.createOscillator();
      const g = ctx!.createGain();
      o.type = "triangle";
      o.frequency.value = f;
      const t = t0 + i * 0.06;
      g.gain.setValueAtTime(0, t);
      g.gain.linearRampToValueAtTime(1, t + 0.01);
      g.gain.exponentialRampToValueAtTime(0.001, t + 0.35);
      o.connect(g).connect(master);
      o.start(t);
      o.stop(t + 0.4);
    });
  } catch {
    /* fără sunet dacă WebAudio nu e disponibil */
  }
}
