<script setup>
import { RadioGroupRoot } from 'reka-ui'
import { cn } from '@/components/shadcn_ui/lib/utils'
const props = defineProps({
    modelValue: { type: String, default: undefined },
    disabled: Boolean,
    orientation: { type: String, default: 'vertical' },
    class: { type: [String, Array, Object], default: '' },
})
const emits = defineEmits(['update:modelValue'])
</script>

<template>
    <RadioGroupRoot
        :model-value="props.modelValue"
        :disabled="props.disabled"
        :orientation="props.orientation"
        :class="cn('grid gap-1.5', props.class)"
        @update:model-value="emits('update:modelValue', $event)"
    >
        <slot />
    </RadioGroupRoot>
</template>
