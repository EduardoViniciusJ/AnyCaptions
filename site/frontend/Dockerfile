FROM node:24-alpine AS build
WORKDIR /app
# Astro inlines PUBLIC_* at build time; the production API is the default
# because Dokploy's Dockerfile builder has no build-args field.
ARG PUBLIC_API_URL=https://api.anycaptions.app/api
ENV PUBLIC_API_URL=$PUBLIC_API_URL
COPY package*.json ./
RUN npm ci
COPY . .
RUN npm run build

FROM node:24-alpine
WORKDIR /app
ENV HOST=0.0.0.0 PORT=4321 NODE_ENV=production
COPY --from=build /app/dist ./dist
COPY --from=build /app/package.json ./package.json
COPY --from=build /app/node_modules ./node_modules
USER node
EXPOSE 4321
CMD ["node", "./dist/server/entry.mjs"]
