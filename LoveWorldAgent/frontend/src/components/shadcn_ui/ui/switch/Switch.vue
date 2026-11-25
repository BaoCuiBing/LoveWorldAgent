<script setup>
import { SwitchRoot, SwitchThumb } from 'reka-ui'
import { cn } from '@/components/shadcn_ui/lib/utils'
const props = defineProps({
    modelValue: { type: Boolean, default: false },
    disabled: Boolean,
    class: { type: [String, Array, Object], default: '' },
})
const emits = defineEmits(['update:modelValue'])
</script>

<template>
    <SwitchRoot
        :model-value="props.modelValue"
        :disabled="props.disabled"
        :class="cn('inline-flex h-4 w-7 shrink-0 cursor-pointer items-center rounded-full border border-transparent bg-input p-0.5 transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50 data-[state=checked]:bg-primary', props.class)"
        @update:model-value="emits('update:modelValue', $event)"
    >
        <SwitchThumb class="block size-3 rounded-full bg-white shadow-sm transition-transform data-[state=checked]:translate-x-3" />
    </SwitchRoot>
</template>
