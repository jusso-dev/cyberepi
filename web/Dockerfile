FROM node:22-bookworm-slim AS build
WORKDIR /src
COPY web/package.json web/package-lock.json ./
RUN npm ci
COPY web ./
RUN npm run build

FROM node:22-bookworm-slim
WORKDIR /app
ENV NODE_ENV=production HOSTNAME=0.0.0.0 PORT=3000
RUN useradd --system --uid 65532 nextjs
COPY --from=build /src/public ./public
COPY --from=build /src/.next/standalone ./
COPY --from=build /src/.next/static ./.next/static
USER 65532:65532
EXPOSE 3000
CMD ["node", "server.js"]
