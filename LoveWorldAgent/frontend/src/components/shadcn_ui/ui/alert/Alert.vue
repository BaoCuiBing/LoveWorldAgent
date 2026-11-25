<script>
import { cva } from 'class-variance-authority'
// 提示条视觉变体
export const alertVariants = cva(
    'relative flex w-full gap-2 rounded-lg border p-2.5 text-xs [&>svg]:size-3.5 [&>svg]:shrink-0 [&>svg]:translate-y-px',
    {
        variants: {
            variant: {
                default: 'border-border bg-card text-card-foreground',
                destructive: 'border-destructive/40 bg-destructive/10 text-destructive [&>svg]:text-destructive',
            },
        },
        defaultVariants: {
            variant: 'default',
        },
    },
)
</script>

<script setup>
import { computed } from 'vue'
import { cn } from '@/components/shadcn_ui/lib/utils'
const props = defineProps({
    variant: { type: String, default: 'default' },
    class: { type: [String, Array, Object], default: '' },
})
const classes = computed(() => cn(alertVariants({ variant: props.variant }), props.class))
</script>

<template>
    <div role="alert" :class="classes">
        <slot />
    </div>
</template>
