<script>
import { cva } from 'class-variance-authority'
// 按钮视觉变体：尺寸偏紧凑，接近 Element Plus 的 small 档位
export const buttonVariants = cva(
    'inline-flex shrink-0 items-center justify-center gap-1.5 whitespace-nowrap rounded-md text-xs font-medium transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50 [&_svg]:pointer-events-none [&_svg]:size-3.5 [&_svg]:shrink-0',
    {
        variants: {
            variant: {
                default: 'bg-primary text-primary-foreground shadow-sm hover:bg-primary/90',
                destructive: 'bg-destructive text-destructive-foreground shadow-sm hover:bg-destructive/90',
                outline: 'border border-input bg-transparent text-foreground hover:bg-accent hover:text-accent-foreground',
                secondary: 'bg-secondary text-secondary-foreground hover:bg-secondary/80',
                ghost: 'text-foreground hover:bg-accent hover:text-accent-foreground',
                link: 'text-primary underline-offset-4 hover:underline',
            },
            size: {
                default: 'h-8 px-3',
                sm: 'h-7 rounded-md px-2.5',
                xs: 'h-6 rounded px-2 text-[11px]',
                lg: 'h-9 rounded-md px-4 text-sm',
                icon: 'size-8',
                'icon-sm': 'size-7 rounded-md',
            },
        },
        defaultVariants: {
            variant: 'default',
            size: 'default',
        },
    },
)
</script>

<script setup>
import { computed } from 'vue'
import { cn } from '@/components/shadcn_ui/lib/utils'
const props = defineProps({
    variant: { type: String, default: 'default' },
    size: { type: String, default: 'default' },
    as: { type: [String, Object], default: 'button' },
    disabled: Boolean,
    class: { type: [String, Array, Object], default: '' },
})
const classes = computed(() => cn(buttonVariants({ variant: props.variant, size: props.size }), props.class))
</script>

<template>
    <component :is="as" :class="classes" :disabled="props.as === 'button' ? props.disabled : undefined">
        <slot />
    </component>
</template>
