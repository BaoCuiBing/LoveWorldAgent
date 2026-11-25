<script setup>
import { ContextMenuCheckboxItem, ContextMenuItemIndicator } from 'reka-ui'
import { Check } from 'lucide-vue-next'
import { cn } from '@/components/shadcn_ui/lib/utils'
const props = defineProps({
    modelValue: { type: [Boolean, String], default: false },
    disabled: Boolean,
    class: { type: [String, Array, Object], default: '' },
})
const emits = defineEmits(['update:modelValue', 'select'])
</script>

<template>
    <ContextMenuCheckboxItem
        :model-value="props.modelValue"
        :disabled="props.disabled"
        :class="cn('relative flex cursor-default select-none items-center gap-2 rounded-sm py-1.5 pl-7 pr-2 text-xs outline-none transition-colors focus:bg-accent focus:text-accent-foreground data-[disabled]:pointer-events-none data-[disabled]:opacity-50', props.class)"
        @update:model-value="emits('update:modelValue', $event)"
        @select="emits('select', $event)"
    >
        <span class="absolute left-1.5 flex size-4 items-center justify-center">
            <ContextMenuItemIndicator>
                <Check class="size-3.5" />
            </ContextMenuItemIndicator>
        </span>
        <slot />
    </ContextMenuCheckboxItem>
</template>
