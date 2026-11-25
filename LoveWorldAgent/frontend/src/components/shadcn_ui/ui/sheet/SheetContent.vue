<script>
import { cva } from 'class-variance-authority'
// 抽屉方向变体：贴住窗口四边，尺寸保持紧凑
export const sheetVariants = cva(
    'fixed z-50 flex flex-col gap-3 border-border bg-popover p-4 text-popover-foreground shadow-xl transition ease-in-out data-[state=open]:animate-in data-[state=closed]:animate-out',
    {
        variants: {
            side: {
                right: 'inset-y-0 right-0 h-full w-80 border-l data-[state=open]:slide-in-from-right data-[state=closed]:slide-out-to-right',
                left: 'inset-y-0 left-0 h-full w-80 border-r data-[state=open]:slide-in-from-left data-[state=closed]:slide-out-to-left',
                top: 'inset-x-0 top-0 h-auto border-b data-[state=open]:slide-in-from-top data-[state=closed]:slide-out-to-top',
                bottom: 'inset-x-0 bottom-0 h-auto border-t data-[state=open]:slide-in-from-bottom data-[state=closed]:slide-out-to-bottom',
            },
        },
        defaultVariants: {
            side: 'right',
        },
    },
)
</script>

<script setup>
import { DialogClose, DialogContent, DialogOverlay, DialogPortal } from 'reka-ui'
import { X } from 'lucide-vue-next'
import { cn } from '@/components/shadcn_ui/lib/utils'
const props = defineProps({
    side: { type: String, default: 'right' },
    showClose: { type: Boolean, default: true },
    class: { type: [String, Array, Object], default: '' },
})
</script>

<template>
    <DialogPortal>
        <DialogOverlay class="fixed inset-0 z-50 bg-black/70 data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=closed]:animate-out data-[state=closed]:fade-out-0" />
        <DialogContent :class="cn(sheetVariants({ side: props.side }), props.class)">
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
