import type { InjectionKey, Ref } from 'vue';

// The shell owns the responsive decision; portaled panels consume the same state.
export const mobileRailLayoutKey: InjectionKey<Readonly<Ref<boolean>>> = Symbol('mobileRailLayout');
