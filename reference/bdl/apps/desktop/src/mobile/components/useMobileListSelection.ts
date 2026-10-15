import { ref } from 'vue';

export function useMobileListSelection(clear: () => void) {
  const managing = ref(false);
  const toggleManaging = () => {
    if (managing.value) clear();
    managing.value = !managing.value;
  };
  const select = (action: () => void) => {
    managing.value = true;
    action();
  };
  return { managing, toggleManaging, select };
}
