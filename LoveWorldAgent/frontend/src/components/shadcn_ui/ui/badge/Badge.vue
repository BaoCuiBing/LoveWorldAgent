<script>
import { cva } from 'class-variance-authority'
// 徽标视觉变体：默认紧凑尺寸
export const badgeVariants = cva(
    'inline-flex items-center gap-1 rounded border px-1.5 py-px text-[11px] font-medium leading-4 transition-colors focus-visible:outline-none',
    {
        variants: {
            variant: {
                default: 'border-transparent bg-primary text-primary-foreground',
                secondary: 'border-transparent bg-secondary text-secondary-foreground',
                destructive: 'border-transparent bg-destructive text-destructive-foreground',
                outline: 'border-border text-foreground',
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
    as: { type: [String, Object], default: 'span' },
    class: { type: [String, Array, Object], default: '' },
})
const classes = computed(() => cn(badgeVariants({ variant: props.variant }), props.class))
</script>

<template>
    <component :is="props.as" :class="classes">
        <slot />
    </component>
</template>
