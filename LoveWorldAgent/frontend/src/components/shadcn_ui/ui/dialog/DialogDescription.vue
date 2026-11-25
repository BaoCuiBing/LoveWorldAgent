<script setup>
import { DialogDescription } from 'reka-ui'
import { cn } from '@/components/shadcn_ui/lib/utils'
const props = defineProps({
    class: { type: [String, Array, Object], default: '' },
})
</script>

<template>
    <DialogDescription :class="cn('text-xs leading-5 text-muted-foreground', props.class)">
        <slot />
    </DialogDescription>
</template>
