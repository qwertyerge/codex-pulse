import { ref } from "vue";

export function useMonotonicClock() {
  const nowMs = ref(Date.now());
  let timer: ReturnType<typeof setTimeout> | undefined;

  const update = () => {
    nowMs.value = Math.max(nowMs.value, Date.now());
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
