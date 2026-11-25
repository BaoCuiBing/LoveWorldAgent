<script setup>
import { DropdownMenuPortal, DropdownMenuSubContent } from 'reka-ui'
import { cn } from '@/components/shadcn_ui/lib/utils'
const props = defineProps({
    class: { type: [String, Array, Object], default: '' },
})
</script>

<template>
    <DropdownMenuPortal>
        <DropdownMenuSubContent
            :side-offset="2"
            :class="cn('z-50 min-w-32 overflow-hidden rounded-md border border-border bg-popover p-1 text-popover-foreground shadow-lg data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=open]:zoom-in-95', props.class)"
        >
            <slot />
        </DropdownMenuSubContent>
    </DropdownMenuPortal>
</template>
