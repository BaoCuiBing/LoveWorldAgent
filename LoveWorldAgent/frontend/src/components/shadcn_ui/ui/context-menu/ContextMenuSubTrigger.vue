<script setup>
import { ContextMenuSubTrigger } from 'reka-ui'
import { ChevronRight } from 'lucide-vue-next'
import { cn } from '@/components/shadcn_ui/lib/utils'
const props = defineProps({
    disabled: Boolean,
    class: { type: [String, Array, Object], default: '' },
})
</script>

<template>
    <ContextMenuSubTrigger
        :disabled="props.disabled"
        :class="cn('flex cursor-default select-none items-center gap-2 rounded-sm px-2 py-1.5 text-xs outline-none transition-colors focus:bg-accent focus:text-accent-foreground data-[state=open]:bg-accent data-[disabled]:pointer-events-none data-[disabled]:opacity-50', props.class)"
    >
        <slot />
        <ChevronRight class="ml-auto size-3.5" />
    </ContextMenuSubTrigger>
</template>
