<script setup>
import { DialogTrigger } from 'reka-ui'
const props = defineProps({
    as: { type: [String, Object], default: 'button' },
    asChild: Boolean,
})
</script>

<template>
    <DialogTrigger :as="props.as" :as-child="props.asChild">
        <slot />
    </DialogTrigger>
</template>
