import { ref } from "vue";

export function useMonotonicClock() {
  const initialWallTime = Date.now();
  const initialMonotonicTime = performance.now();
  const nowMs = ref(initialWallTime);
  let timer: ReturnType<typeof setTimeout> | undefined;

  const update = () => {
    nowMs.value = initialWallTime + performance.now() - initialMonotonicTime;
    const delay = 1_000 - (Math.floor(nowMs.value) % 1_000);
    timer = setTimeout(update, Math.max(1, delay));
  };

  return {
    nowMs,
    start: update,
    stop: () => {
      if (timer) clearTimeout(timer);
      timer = undefined;
    }
  };
}
