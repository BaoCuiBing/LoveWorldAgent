<script setup>
import { CheckboxIndicator, CheckboxRoot } from 'reka-ui'
import { Check, Minus } from 'lucide-vue-next'
import { cn } from '@/components/shadcn_ui/lib/utils'
const props = defineProps({
    modelValue: { type: [Boolean, String], default: false },
    disabled: Boolean,
    class: { type: [String, Array, Object], default: '' },
})
const emits = defineEmits(['update:modelValue'])
</script>

<template>
    <CheckboxRoot
        :model-value="props.modelValue"
        :disabled="props.disabled"
        :class="cn('peer size-3.5 shrink-0 rounded-sm border border-input shadow-sm outline-none transition-colors focus-visible:ring-1 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50 data-[state=checked]:border-primary data-[state=checked]:bg-primary data-[state=checked]:text-primary-foreground data-[state=indeterminate]:border-primary data-[state=indeterminate]:bg-primary data-[state=indeterminate]:text-primary-foreground', props.class)"
        @update:model-value="emits('update:modelValue', $event)"
    >
        <CheckboxIndicator class="flex items-center justify-center text-current">
            <Minus v-if="props.modelValue === 'indeterminate'" class="size-3" />
            <Check v-else class="size-3" />
        </CheckboxIndicator>
    </CheckboxRoot>
</template>
