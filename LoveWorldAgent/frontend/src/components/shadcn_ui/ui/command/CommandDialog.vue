<script setup>
import { DialogRoot, DialogContent, DialogPortal, DialogTitle, DialogDescription } from 'reka-ui'
import { cn } from '@/components/shadcn_ui/lib/utils'
import Command from './Command.vue'
const props = defineProps({
    open: { type: Boolean, default: undefined },
    title: { type: String, default: '命令面板' },
    description: { type: String, default: '输入关键字搜索可用命令' },
    class: { type: [String, Array, Object], default: '' },
})
const emits = defineEmits(['update:open'])
</script>

<template>
    <DialogRoot :open="props.open" @update:open="emits('update:open', $event)">
        <DialogPortal>
            <DialogContent
                :class="cn('fixed left-1/2 top-1/3 z-50 w-full max-w-md -translate-x-1/2 overflow-hidden rounded-lg border border-border bg-popover p-0 text-popover-foreground shadow-xl data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=open]:zoom-in-95', props.class)"
            >
                <DialogTitle class="sr-only">{{ props.title }}</DialogTitle>
                <DialogDescription class="sr-only">{{ props.description }}</DialogDescription>
                <Command>
                    <slot />
                </Command>
            </DialogContent>
        </DialogPortal>
    </DialogRoot>
</template>
