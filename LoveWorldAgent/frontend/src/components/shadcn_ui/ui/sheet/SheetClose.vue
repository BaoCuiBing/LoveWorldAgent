<script setup>
import { DialogClose } from 'reka-ui'
import { cn } from '@/components/shadcn_ui/lib/utils'
const props = defineProps({
    as: { type: [String, Object], default: 'button' },
    asChild: Boolean,
    class: { type: [String, Array, Object], default: '' },
})
</script>

<template>
    <DialogClose :as="props.as" :as-child="props.asChild" :class="cn(props.class)">
        <slot />
    </DialogClose>
</template>
