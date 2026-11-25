<script setup>
import { SelectIcon, SelectTrigger } from 'reka-ui'
import { ChevronDown } from 'lucide-vue-next'
import { cn } from '@/components/shadcn_ui/lib/utils'
const props = defineProps({
    class: { type: [String, Array, Object], default: '' },
    disabled: Boolean,
})
</script>

<template>
    <SelectTrigger
        :disabled="props.disabled"
        :class="cn('flex h-8 w-full items-center justify-between gap-2 rounded-md border border-input bg-transparent px-2.5 py-1 text-xs text-foreground shadow-sm outline-none transition-colors data-[placeholder]:text-muted-foreground focus-visible:ring-1 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50', props.class)"
    >
        <slot />
        <SelectIcon class="text-muted-foreground">
            <ChevronDown class="size-3.5" />
        </SelectIcon>
    </SelectTrigger>
</template>
