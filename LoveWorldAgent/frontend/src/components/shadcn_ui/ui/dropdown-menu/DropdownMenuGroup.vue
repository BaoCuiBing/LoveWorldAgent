<script setup>
import { DropdownMenuGroup } from 'reka-ui'
const props = defineProps({
    as: { type: [String, Object], default: 'div' },
})
</script>

<template>
    <DropdownMenuGroup :as="props.as">
        <slot />
    </DropdownMenuGroup>
</template>
