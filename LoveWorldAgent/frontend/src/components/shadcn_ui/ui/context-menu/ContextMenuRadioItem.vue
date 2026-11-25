<script setup>
import { ContextMenuItemIndicator, ContextMenuRadioItem } from 'reka-ui'
import { Circle } from 'lucide-vue-next'
import { cn } from '@/components/shadcn_ui/lib/utils'
const props = defineProps({
    value: { type: String, required: true },
    disabled: Boolean,
    class: { type: [String, Array, Object], default: '' },
})
</script>

<template>
    <ContextMenuRadioItem
        :value="props.value"
        :disabled="props.disabled"
        :class="cn('relative flex cursor-default select-none items-center gap-2 rounded-sm py-1.5 pl-7 pr-2 text-xs outline-none transition-colors focus:bg-accent focus:text-accent-foreground data-[disabled]:pointer-events-none data-[disabled]:opacity-50', props.class)"
    >
        <span class="absolute left-1.5 flex size-4 items-center justify-center">
            <ContextMenuItemIndicator>
                <Circle class="size-2 fill-current" />
            </ContextMenuItemIndicator>
        </span>
        <slot />
    </ContextMenuRadioItem>
</template>
