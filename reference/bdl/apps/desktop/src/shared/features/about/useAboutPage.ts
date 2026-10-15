import { ref } from 'vue';
import { getVersion } from '@tauri-apps/api/app';
import { onMounted } from 'vue';
import { openExternalUrl } from '../../api/tauri';
import { useUiStore } from '../../stores/ui';

export function useAboutPage() {
  const ui = useUiStore();
  const version = ref('0.4.0');
  const links = [
    {
      label: 'GitHub 仓库',
      value: 'github.com/daodeqing/CutFlow',
      url: 'https://github.com/daodeqing/CutFlow',
      icon: 'i-tabler-brand-github',
    },
    {
      label: 'BDL 原项目 · MIT',
      value: 'github.com/Yuelioi/bdl',
      url: 'https://github.com/Yuelioi/bdl',
      icon: 'i-tabler-brand-bilibili',
    },
    {
      label: 'Concat 原项目 · AGPL-3.0-or-later',
      value: 'github.com/jub0t/concat',
      url: 'https://github.com/jub0t/concat',
      icon: 'i-tabler-world-www',
    },
  ];
  const openLink = async (url: string) => {
    try {
      await openExternalUrl(url);
    } catch (error) {
      ui.pushToast(error instanceof Error ? error.message : String(error), 'danger');
    }
  };
  onMounted(async () => {
    try {
      version.value = await getVersion();
    } catch {
      // The package version remains available in browser-only previews.
    }
  });
  return { version, links, openLink };
}
