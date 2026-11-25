<script setup>
import { DialogClose, DialogContent, DialogPortal } from 'reka-ui'
import { X } from 'lucide-vue-next'
import { cn } from '@/components/shadcn_ui/lib/utils'
import DialogOverlay from './DialogOverlay.vue'
const props = defineProps({
    showClose: { type: Boolean, default: true },
    class: { type: [String, Array, Object], default: '' },
    overlayClass: { type: [String, Array, Object], default: '' },
})
</script>

<template>
    <DialogPortal>
        <DialogOverlay :class="props.overlayClass" />
        <DialogContent
            :class="cn('fixed left-1/2 top-1/2 z-50 grid w-full max-w-lg -translate-x-1/2 -translate-y-1/2 gap-3 rounded-lg border border-border bg-popover p-4 text-popover-foreground shadow-xl data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=open]:zoom-in-95 data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=closed]:zoom-out-95', props.class)"
        >
            <slot />
            <DialogClose
                v-if="props.showClose"
                class="absolute right-3 top-3 rounded-sm text-muted-foreground opacity-80 transition-opacity hover:opacity-100 focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
            >
                <X class="size-3.5" />
                <span class="sr-only">关闭</span>
            </DialogClose>
        </DialogContent>
    </DialogPortal>
</template>
